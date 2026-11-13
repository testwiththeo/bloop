// Portal session setup for XDG ScreenCast.
//
// Flow mirrors Kooha, the reference ScreenCast implementation:
// src/screencast_portal/mod.rs — create_session sends handle_token +
// session_handle_token, select_sources passes types/multiple/
// cursor_mode/persist_mode/restore_token, start returns streams,
// open_pipe_wire_remote returns an OwnedFd, Session::close calls
// org.freedesktop.portal.Session.Close. Flag and field definitions in
// src/screencast_portal/types.rs, tokens in handle_token.rs, the parent
// window identifier in window_identifier.rs (None here for now). ashpd
// calls 1:1 (docs.rs/ashpd/latest/ashpd/desktop/screencast):
// Screencast::new -> create_session -> select_sources -> start ->
// Streams -> open_pipe_wire_remote; Session::close ends it.

use crate::pipeline::{Pipeline, PipelineError};
use ashpd::desktop::screencast::{
    CursorMode, OpenPipeWireRemoteOptions, Screencast, SelectSourcesOptions, SourceType,
    StartCastOptions,
};
use ashpd::desktop::{CreateSessionOptions, PersistMode, ResponseError, Session};
use gtk4::glib::MainContext;
use std::os::fd::OwnedFd;

#[derive(Debug)]
pub enum CaptureError {
    /// The user dismissed the portal dialog (ResponseError::Cancelled).
    Cancelled,
    NoStream,
    Portal(ashpd::Error),
    Pipeline(PipelineError),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "user cancelled the portal dialog"),
            Self::NoStream => write!(f, "portal returned no PipeWire stream"),
            Self::Portal(e) => write!(f, "portal error: {e}"),
            Self::Pipeline(e) => write!(f, "pipeline error: {e}"),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<ashpd::Error> for CaptureError {
    fn from(e: ashpd::Error) -> Self {
        match e {
            ashpd::Error::Response(ResponseError::Cancelled) => Self::Cancelled,
            other => Self::Portal(other),
        }
    }
}

/// The portal calls start_portal_session performs, in Kooha's order
/// (screencast_portal/mod.rs). Split out so tests can drive a mock.
pub trait PortalBackend {
    type Session;

    async fn create_session(&self) -> Result<Self::Session, CaptureError>;
    async fn select_sources(&self, session: &Self::Session) -> Result<(), CaptureError>;
    async fn start(&self, session: &Self::Session) -> Result<u32, CaptureError>;
    async fn open_pipe_wire_remote(&self, session: &Self::Session)
        -> Result<OwnedFd, CaptureError>;
    async fn close_session(&self, session: &Self::Session) -> Result<(), CaptureError>;
}

/// A live portal session; the remote fd stays open until close() or drop.
pub struct PortalSession<B: PortalBackend> {
    backend: B,
    session: B::Session,
    node_id: u32,
    pipewire_fd: OwnedFd,
}

impl<B: PortalBackend> PortalSession<B> {
    pub fn pipewire_node_id(&self) -> u32 {
        self.node_id
    }

    pub fn pipewire_fd(&self) -> &OwnedFd {
        &self.pipewire_fd
    }

    /// Close org.freedesktop.portal.Session (Kooha Session::close).
    pub async fn close(self) -> Result<(), CaptureError> {
        self.backend.close_session(&self.session).await
    }
}

/// Runs the portal handshake on org.freedesktop.portal.ScreenCast and
/// returns the live session with its PipeWire stream node id.
pub async fn start_portal_session() -> Result<PortalSession<AshpdPortal>, CaptureError> {
    start_portal_session_with(AshpdPortal::connect().await?).await
}

async fn start_portal_session_with<B: PortalBackend>(
    backend: B,
) -> Result<PortalSession<B>, CaptureError> {
    let session = backend.create_session().await?;
    // Later failures still close the session so the portal dialog and
    // its screencast state do not leak (Kooha Session::close).
    let result = async {
        backend.select_sources(&session).await?;
        let node_id = backend.start(&session).await?;
        let fd = backend.open_pipe_wire_remote(&session).await?;
        Ok((node_id, fd))
    }
    .await;
    match result {
        Ok((node_id, pipewire_fd)) => Ok(PortalSession {
            backend,
            session,
            node_id,
            pipewire_fd,
        }),
        Err(e) => {
            let _ = backend.close_session(&session).await;
            Err(e)
        }
    }
}

/// A recording in progress: the live portal session and the running
/// GStreamer pipeline fed by its PipeWire remote. Both halves must be
/// kept and stopped together, so they live in one struct.
pub struct Recording<B: PortalBackend, P> {
    session: PortalSession<B>,
    pipeline: P,
}

/// Anything that can be shut down before the portal session closes;
/// pipeline::Pipeline implements it via Pipeline::stop. The trait is
/// the test seam so a mock pipeline can record its stop call.
pub trait Stoppable {
    fn stop(&self) -> Result<(), PipelineError>;
}

impl Stoppable for Pipeline {
    fn stop(&self) -> Result<(), PipelineError> {
        Pipeline::stop(self)
    }
}

impl<B: PortalBackend, P> Recording<B, P> {
    pub fn session(&self) -> &PortalSession<B> {
        &self.session
    }

    pub fn pipeline(&self) -> &P {
        &self.pipeline
    }
}

impl<B: PortalBackend, P: Stoppable> Recording<B, P> {
    /// Stops the GStreamer pipeline first so PipeWire disconnects, then
    /// closes the portal session (Kooha tears down its encoder before
    /// Session::close). A pipeline failure does not skip the close.
    pub fn stop(self) -> Result<(), CaptureError> {
        let pipeline_result = self.pipeline.stop().map_err(CaptureError::Pipeline);
        let close_result = MainContext::default().block_on(self.session.close());
        pipeline_result.and(close_result)
    }
}

/// Runs the portal handshake, then starts the pipewiresrc pipeline on
/// the returned fd and node id. This is the Kooha screencast flow's
/// second half: screencast_portal hands the remote fd + stream node id
/// to src/pipeline.rs make_pipewiresrc.
pub async fn start_recording() -> Result<Recording<AshpdPortal, Pipeline>, CaptureError> {
    let session = start_portal_session().await?;
    match crate::pipeline::start_pipeline(session.pipewire_fd(), session.pipewire_node_id()) {
        Ok(pipeline) => Ok(Recording { session, pipeline }),
        Err(e) => {
            let _ = session.close().await;
            Err(CaptureError::Pipeline(e))
        }
    }
}

async fn start_recording_with<B: PortalBackend, P>(
    backend: B,
    start_pipeline: impl FnOnce(&OwnedFd, u32) -> Result<P, PipelineError>,
) -> Result<Recording<B, P>, CaptureError> {
    let session = start_portal_session_with(backend).await?;
    match start_pipeline(session.pipewire_fd(), session.pipewire_node_id()) {
        Ok(pipeline) => Ok(Recording { session, pipeline }),
        Err(e) => {
            let _ = session.close().await;
            Err(CaptureError::Pipeline(e))
        }
    }
}

/// ashpd 0.14.3 backend over org.freedesktop.portal.ScreenCast.
pub struct AshpdPortal {
    proxy: Screencast,
}

impl AshpdPortal {
    async fn connect() -> Result<Self, CaptureError> {
        Ok(Self {
            proxy: Screencast::new().await?,
        })
    }
}

impl PortalBackend for AshpdPortal {
    type Session = Session<Screencast>;

    async fn create_session(&self) -> Result<Self::Session, CaptureError> {
        Ok(self
            .proxy
            .create_session(CreateSessionOptions::default())
            .await?)
    }

    async fn select_sources(&self, session: &Self::Session) -> Result<(), CaptureError> {
        // Same options Kooha Session::select_sources sends; EMBEDDED
        // bakes the cursor into frames so bug-report GIFs show where
        // the pointer was.
        self.proxy
            .select_sources(
                session,
                SelectSourcesOptions::default()
                    .set_cursor_mode(CursorMode::EMBEDDED)
                    .set_sources(SourceType::MONITOR | SourceType::WINDOW | SourceType::VIRTUAL)
                    .set_multiple(true)
                    .set_persist_mode(PersistMode::DoNot),
            )
            .await?;
        Ok(())
    }

    async fn start(&self, session: &Self::Session) -> Result<u32, CaptureError> {
        // No parent window identifier yet; Kooha builds one in
        // screencast_portal/window_identifier.rs from the GTK4 native.
        let streams = self
            .proxy
            .start(session, None, StartCastOptions::default())
            .await?
            .response()?;
        let stream = streams.streams().first().ok_or(CaptureError::NoStream)?;
        Ok(stream.pipe_wire_node_id())
    }

    async fn open_pipe_wire_remote(
        &self,
        session: &Self::Session,
    ) -> Result<OwnedFd, CaptureError> {
        Ok(self
            .proxy
            .open_pipe_wire_remote(session, OpenPipeWireRemoteOptions::default())
            .await?)
    }

    async fn close_session(&self, session: &Self::Session) -> Result<(), CaptureError> {
        Ok(session.close().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::fs::File;
    use std::os::fd::AsRawFd;
    use std::rc::Rc;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    // Minimal std-only executor: mock futures resolve on first poll.
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        fn clone(_: *const ()) -> RawWaker {
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        fn noop(_: *const ()) {}
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut fut = std::pin::pin!(fut);
        loop {
            if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
                return v;
            }
            std::thread::yield_now();
        }
    }

    struct MockSession {
        closed: Cell<bool>,
    }

    struct MockPortal {
        calls: Rc<RefCell<Vec<&'static str>>>,
        fail_at: Option<&'static str>,
    }

    impl PortalBackend for MockPortal {
        type Session = MockSession;

        async fn create_session(&self) -> Result<MockSession, CaptureError> {
            self.calls.borrow_mut().push("create_session");
            if self.fail_at == Some("create_session") {
                return Err(CaptureError::NoStream);
            }
            Ok(MockSession {
                closed: Cell::new(false),
            })
        }

        async fn select_sources(&self, _s: &MockSession) -> Result<(), CaptureError> {
            self.calls.borrow_mut().push("select_sources");
            if self.fail_at == Some("select_sources") {
                return Err(CaptureError::NoStream);
            }
            Ok(())
        }

        async fn start(&self, _s: &MockSession) -> Result<u32, CaptureError> {
            self.calls.borrow_mut().push("start");
            if self.fail_at == Some("start") {
                return Err(CaptureError::NoStream);
            }
            Ok(42)
        }

        async fn open_pipe_wire_remote(&self, _s: &MockSession) -> Result<OwnedFd, CaptureError> {
            self.calls.borrow_mut().push("open_pipe_wire_remote");
            Ok(File::open("/dev/null").unwrap().into())
        }

        async fn close_session(&self, s: &MockSession) -> Result<(), CaptureError> {
            self.calls.borrow_mut().push("close");
            s.closed.set(true);
            Ok(())
        }
    }

    #[test]
    fn portal_session_created_and_closed_cleanly() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mock = MockPortal {
            calls: calls.clone(),
            fail_at: None,
        };
        let session = block_on(start_portal_session_with(mock)).unwrap();
        assert_eq!(session.pipewire_node_id(), 42);
        assert!(session.pipewire_fd().as_raw_fd() >= 0);
        assert_eq!(
            calls.borrow().as_slice(),
            [
                "create_session",
                "select_sources",
                "start",
                "open_pipe_wire_remote"
            ]
        );
        block_on(session.close()).unwrap();
        assert_eq!(calls.borrow().last().copied(), Some("close"));
    }

    #[test]
    fn portal_session_failure_closes_the_session() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mock = MockPortal {
            calls: calls.clone(),
            fail_at: Some("start"),
        };
        let result = block_on(start_portal_session_with(mock));
        assert!(matches!(result, Err(CaptureError::NoStream)));
        assert_eq!(calls.borrow().last().copied(), Some("close"));
    }

    struct MockPipeline {
        fd_seen: i32,
        node_seen: u32,
    }

    #[test]
    fn recording_passes_fd_and_node_id_to_pipeline() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mock = MockPortal {
            calls: calls.clone(),
            fail_at: None,
        };
        let recording = block_on(start_recording_with(mock, |fd, node_id| {
            Ok(MockPipeline {
                fd_seen: fd.as_raw_fd(),
                node_seen: node_id,
            })
        }))
        .unwrap();
        assert_eq!(recording.pipeline().node_seen, 42);
        assert_eq!(
            recording.pipeline().fd_seen,
            recording.session().pipewire_fd().as_raw_fd()
        );
        assert_eq!(
            calls.borrow().as_slice(),
            [
                "create_session",
                "select_sources",
                "start",
                "open_pipe_wire_remote"
            ]
        );
    }

    #[test]
    fn pipeline_failure_closes_the_session() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mock = MockPortal {
            calls: calls.clone(),
            fail_at: None,
        };
        let result: Result<Recording<MockPortal, MockPipeline>, _> =
            block_on(start_recording_with(mock, |_, _| {
                Err(PipelineError::MissingCaps)
            }));
        assert!(matches!(result, Err(CaptureError::Pipeline(_))));
        assert_eq!(calls.borrow().last().copied(), Some("close"));
    }

    struct LoggedPipeline {
        log: Rc<RefCell<Vec<&'static str>>>,
        fail_stop: bool,
    }

    impl Stoppable for LoggedPipeline {
        fn stop(&self) -> Result<(), PipelineError> {
            self.log.borrow_mut().push("pipeline_stop");
            if self.fail_stop {
                return Err(PipelineError::MissingCaps);
            }
            Ok(())
        }
    }

    #[test]
    fn stop_halts_pipeline_before_closing_session() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mock = MockPortal {
            calls: calls.clone(),
            fail_at: None,
        };
        let pipeline = LoggedPipeline {
            log: calls.clone(),
            fail_stop: false,
        };
        let recording = block_on(start_recording_with(mock, |_, _| Ok(pipeline))).unwrap();
        recording.stop().unwrap();
        assert_eq!(
            calls.borrow().as_slice(),
            [
                "create_session",
                "select_sources",
                "start",
                "open_pipe_wire_remote",
                "pipeline_stop",
                "close"
            ]
        );
    }

    #[test]
    fn stop_after_pipeline_failure_still_closes_session() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mock = MockPortal {
            calls: calls.clone(),
            fail_at: None,
        };
        let pipeline = LoggedPipeline {
            log: calls.clone(),
            fail_stop: true,
        };
        let recording = block_on(start_recording_with(mock, |_, _| Ok(pipeline))).unwrap();
        let result = recording.stop();
        assert!(matches!(result, Err(CaptureError::Pipeline(_))));
        assert_eq!(calls.borrow().last().copied(), Some("close"));
    }
}
