# ARCHITECTURE.md, How Bloop Works Internally

## 1. Overview

Bloop has a deliberately simple architecture. There are four stages, and data flows in one direction. There is no complex plugin system, no background service, and no server.

```
Portal Picker -> PipeWire Stream -> GStreamer Pipeline -> Frames + Annotations -> gifski -> GIF
```

- Portal: Asks the system for permission and lets the user pick what to record. This is the only correct way to record on Wayland.
- PipeWire: The system service that provides the raw video stream.
- GStreamer: Reads the PipeWire stream and produces frames.
- Frames + Annotations: Frames are stored in memory or temp files. Annotations are stored separately in `.bloop.json`.
- gifski: Encodes the composited frames into a GIF.

The key architectural decision is: we do not encode to GIF during recording. We store raw frames first, let the user edit and annotate, and only encode at export time. This makes annotation non destructive and lets the user change FPS and scale without re-recording.

## 2. Modules

### `src/main.rs`, Entry Point

- Initializes GTK4, LibAdwaita, and GStreamer.
- Parses CLI args, in v1 only `--help` and `--version`.
- Starts the main window.

### `src/capture.rs`, The Hard Part

This is the most complex and most fragile module. It owns:

- Portal session: Uses the `ashpd` crate to create a ScreenCast session. Handles the portal async handshake, which involves a request token, a session token, and a file descriptor for the PipeWire stream.
- Stream negotiation: Asks for a stream with the right resolution and format. Handles the case where the user picks a monitor vs a window vs an area.
- GStreamer pipeline: Builds a pipeline like `pipewiresrc ! videoconvert ! videorate ! appsink`. `videorate` is used to enforce the target FPS, 10 to 15. `appsink` delivers frames to Rust code.
- Frame capture: Receives frames from `appsink`, drops duplicates, if a frame is pixel identical to the previous one, it is not stored, and stores the rest with timestamps.

Threading: Capture runs on a separate thread from the UI. The UI must never block on capture. Frames are sent to the UI thread via a channel, throttled to the display refresh rate.

Error cases: If the portal is denied, if PipeWire is not available, or if the stream ends unexpectedly, for example the window closes, `capture.rs` must return a structured error, not panic, and the UI must show a human readable message.

### `src/annotation.rs`, The Safe Playground

This module owns annotation types, drawing, and hit testing.

- Defines the annotation types from DATA_MODEL.md: arrow, box, text, highlight, blur.
- Draws each type with Cairo onto a frame.
- Handles hit testing: when the user clicks on the canvas, which annotation did they click? This is used for move, resize, and delete.
- Handles time: each annotation has `start_ms` and `end_ms`. When the editor shows frame at time T, it draws only annotations where `start_ms <= T <= end_ms`.

This module is deliberately isolated from capture and export. It takes a frame and a list of annotations and returns a composited frame. It does not know about PipeWire or gifski.

### `src/export.rs`, GIF Encoding

This module owns:

- Taking the list of frames plus annotations and producing a GIF.
- Calling `gifski` with the correct settings for FPS, scale, and quality.
- Frame deduplication at export time, in addition to capture time deduplication.
- Size estimation: before the real export, it estimates the file size based on frame count, resolution, and FPS, so the UI can show approx 2.3 MB without encoding twice.
- Clipboard: writes the GIF bytes to the clipboard via the GTK clipboard API.

Why gifski and not FFmpeg? `gifski` produces higher quality GIFs with better palettes than FFmpeg default GIF encoder, and it is a Rust library, so it fits the stack without a runtime dependency. FFmpeg is a general video tool. `gifski` is a GIF specialist, which matches Bloop philosophy.

### `src/ui.rs`, GTK4 and LibAdwaita

This module owns all widgets:

- Main window: Header bar with Record, Stop, Export, and Settings.
- Source picker: Three buttons: Area, Window, Monitor. The actual picker UI is provided by the portal, not by Bloop. Bloop only tells the portal which type to show.
- Timeline: A custom widget that shows the filmstrip, the playhead, and annotation markers. It supports click to seek, drag to trim, and keyboard arrows to move frame by frame.
- Canvas: Shows the current frame with annotations on top. Supports drag to move annotations, and handles to resize boxes.
- Export dialog: FPS, scale, quality, size preview, Save and Copy buttons.

Design principle: The UI should be understandable without a manual. If a button needs a tooltip paragraph to explain it, the button is wrong.

### `src/project.rs`, Project File

- Loads and saves `.bloop.json`.
- Validates the JSON against the schema in DATA_MODEL.md.
- Handles version migration: if a future version adds a field, old files must still load. Unknown fields are ignored, not treated as errors.

## 3. Data Flow in Detail

### Recording Flow

1. User clicks Record, `ui.rs` calls `capture.rs::start()`
2. `capture.rs` creates a portal session via `ashpd`
3. System shows the native picker, user picks Area, Window, or Monitor
4. Portal returns a PipeWire file descriptor
5. `capture.rs` builds a GStreamer pipeline with that FD
6. GStreamer emits frames, `capture.rs` timestamps them, drops duplicates, stores them
7. Frames are sent to `ui.rs` for live preview, throttled
8. User clicks Stop, pipeline is stopped, frames are kept

### Editing Flow

1. User trims, `ui.rs` updates `project.trim_start_ms` and `trim_end_ms`
2. User adds an arrow, `ui.rs` calls `annotation.rs::add()` with the new annotation
3. User drags an annotation in time, `ui.rs` updates `start_ms` and `end_ms`
4. The canvas re-renders the current frame with the current annotations

### Export Flow

1. User clicks Export, `ui.rs` calls `export.rs::estimate_size()`, shows approx 2.3 MB
2. User clicks Save, `export.rs::export()` iterates over frames in the trimmed range, composites annotations with Cairo, feeds frames to `gifski`
3. `gifski` writes the GIF bytes
4. `ui.rs` shows "Saved" and offers to copy to clipboard or open the file location

## 4. Why This Architecture?

### Alternative Considered: Record Directly to GIF

We could have encoded to GIF during recording, like Peek does. This would be simpler and use less memory. But it would make annotation destructive: you cannot move an arrow after the GIF is already encoded. It would also make trimming and FPS changes impossible without re-encoding. We rejected this because annotation is core to Bloop, not an extra.

### Alternative Considered: Use FFmpeg CLI

We could have shelled out to `ffmpeg` for encoding. This would be easier to implement initially, but it would add a runtime dependency that is not guaranteed to be present on all systems, and FFmpeg GIF output is lower quality than `gifski`. We chose `gifski` for quality and for Rust native integration.

### Alternative Considered: Store Frames as PNGs on Disk

Storing every frame as a PNG would use too much disk for a 60 second recording. We store frames in memory for short recordings, the v1 target is under 60 seconds, and spill to a temp directory if memory pressure is high. The temp directory is cleaned up on exit and on successful export if the user does not save the project.

## 5. Performance Considerations

- Frame deduplication: If the screen does not change, for example user is reading, we do not store duplicate frames. This is the single biggest factor in keeping GIFs small.
- Preview throttling: The live preview during recording is throttled to 15 FPS even if the pipeline produces more. The UI does not need 60 FPS.
- Export parallelism: `gifski` can use multiple threads. We allow it, but we cap threads to avoid freezing the UI during export. The UI shows a progress bar during export.
- Memory: At 12 FPS, 800x600, 60 seconds equals 720 frames. Each frame is approx 1.4 MB raw. That is approx 1 GB if stored naively. Deduplication plus scaling plus storing as compressed buffers is essential. In v1, we store frames as `GstBuffer` with a cap, and we warn the user if they record a very large area at 100 percent scale for 60 seconds.

## 6. Security and Privacy

- Bloop never makes network requests.
- Bloop never uploads screen content anywhere.
- The portal permission is requested each time, and the user can revoke it in system settings.
- Blur annotations are applied at export time to the actual GIF bytes, not just as a UI overlay. This is tested: there is a test that exports a GIF with a blur over a known pattern and verifies that the pattern is not recoverable from the GIF.
