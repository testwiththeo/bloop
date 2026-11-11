# SPEC.md, Bloop v1 Specification

## 1. Product Identity

- Name: Bloop
- Display Name: Bloop
- Repo Slug: `bloop-gif`
- Binary: `bloop`
- Flatpak ID: `io.github.open-qa-tools.Bloop`
- Type: Standalone desktop application for Linux
- Scope: GIF only, offline, no account, no cloud, no AI

Bloop is not a platform. It is not a suite. It is not a plugin for another QA tool. It is a single tool that does one job extremely well: produce a small, annotated GIF that proves a bug exists.

## 2. Problem Statement

In a typical software team, the most expensive part of fixing a bug is not fixing it. It is understanding it. A bug report that says "the filter does not work" can mean five different things, and the developer has to ask for clarification, wait for a reply, and try to reproduce it from a vague description. Each round trip costs hours.

Screenshots help, but they are static. They cannot show that a dropdown flickers, that a button does nothing on click, or that a list reorders itself incorrectly. Video helps, but it is heavy: a 30 second screen recording of a full monitor is tens of megabytes, it requires uploading, it requires a player, and it forces the viewer to scrub to find the relevant three seconds.

GIFs are the sweet spot. They autoplay inline in GitHub, Jira, Slack, and email. They loop. They are self contained. But existing Linux GIF tools are either unmaintained, too generic, or produce enormous, low quality GIFs that flicker and stutter.

Bloop exists to make the GIF path the path of least resistance.

## 3. Personas

### Primary: Maya, QA Engineer

Maya files 15 bug reports a day. She needs each one to be unambiguous in under a minute. She does not have time to edit video. She needs to record, annotate, and paste. If a tool takes more than 60 seconds per bug, she will not use it.

Her environment: GNOME on Fedora, 1080p monitor, GitHub Issues for bug tracking.

### Secondary: Ardi, Frontend Developer

Ardi wants to show a 4 second UI change to a designer: "I fixed the hover state, see?" He does not want to write a paragraph. He wants a GIF that loops and shows the change. He cares about visual quality and file size, because he pastes GIFs into PR descriptions.

### Tertiary: Sari, Support Engineer

Sari talks to customers who cannot describe what they did. She needs to record a reproduction, blur the customer email address in the recording, and send it to engineering. Privacy matters to her. She will not use a tool that uploads to a cloud.

## 4. Goals and Non Goals

### Goals for v1

- A new user can go from opening Bloop to exporting a GIF in under 60 seconds
- A 5 second GIF at default settings is under 5 MB
- GIFs are clear, not flickery, and loop smoothly
- Annotations are easy to add, easy to move, and easy to delete
- The app works offline and never asks for an account

### Non Goals for v1, Explicitly Out

- Long form video recording, over 30 seconds is not the target
- Video editing with tracks, transitions, or keyframes
- Cloud upload, share links, or collaboration
- Integrations with Jira, GitHub, Linear, Slack, or any other service
- AI features: auto captions, auto zoom, auto highlight, auto bug description
- Webcam, microphone, or system audio recording

## 5. Functional Requirements

### FR-01: Recording Sources

- FR-01.1: User can record a rectangular area they draw with the mouse.
- FR-01.2: User can record a single window, selected from the system picker.
- FR-01.3: User can record a full monitor, including on multi monitor setups.
- FR-01.4: The recording source picker must use the native portal picker, not a custom fake picker that tries to list windows itself. This is required for Wayland security.

### FR-02: Recording Controls

- FR-02.1: User can start recording with one click.
- FR-02.2: User can pause and resume without splitting the GIF.
- FR-02.3: User can stop recording and go directly to the editor.
- FR-02.4: User can optionally enable a 3 second countdown before recording starts.
- FR-02.5: Recording is capped at 60 seconds in v1. If the user hits 60 seconds, recording stops automatically and shows a message: "Bloop is for short GIFs. Keep it under 60 seconds."

### FR-03: Editing

- FR-03.1: User can trim the start and end with a timeline slider.
- FR-03.2: User can scrub frame by frame to find the exact moment.
- FR-03.3: User can cut out a middle section, for example the part where you were waiting for the app to load.
- FR-03.4: The timeline must show the current frame, total duration, and annotation markers.

### FR-04: Annotation

- FR-04.1: Arrow, point to a specific element, with color and thickness controls.
- FR-04.2: Box, draw a rectangle around a region.
- FR-04.3: Text, add a short label on the GIF. Max 140 characters per label to keep GIFs readable.
- FR-04.4: Highlight, emphasize a region with a translucent color.
- FR-04.5: Blur and Mosaic, hide sensitive data. Must be true blur in the exported GIF, not just a UI overlay that disappears on export.
- FR-04.6: Cursor highlight, optionally draw a subtle circle around the cursor and a ripple on click.
- FR-04.7: Every annotation has a start time and an end time. It appears only in that window. The user can drag the annotation in time on the timeline.
- FR-04.8: Annotations are non destructive. They are stored separately in `.bloop.json` and only baked into the GIF at export. The user can edit or delete them at any time before export.

### FR-05: Export

- FR-05.1: Export is GIF only in v1.
- FR-05.2: User can set FPS: 5, 10, 12, 15, or 20. Default is 12.
- FR-05.3: User can set scale: 25, 50, 75, or 100 percent. Default is 100 percent for small areas, 75 percent for full monitor.
- FR-05.4: User can see a live file size estimate before exporting. The estimate must update when FPS or scale changes.
- FR-05.5: If the estimated size is over 10 MB, the UI must show a warning and suggest lowering FPS or scale.
- FR-05.6: GIF loops forever by default.
- FR-05.7: User can save to a `.gif` file.
- FR-05.8: User can copy the GIF to the clipboard.
- FR-05.9: User can save the project as `.bloop.json` to edit later.

### FR-06: Settings and Persistence

- FR-06.1: Bloop remembers the last export folder, last FPS, and last scale.
- FR-06.2: Bloop does not require an account or any network access.
- FR-06.3: All settings are stored locally via GSettings or a local config file.

## 6. Non Functional Requirements

### NFR-01: Performance

- Recording must not drop below 10 FPS on a typical 1080p machine during normal UI work.
- The editor must remain responsive with up to 900 frames, 60 seconds at 15 FPS.
- Export of a 5 second GIF must complete in under 5 seconds on a typical machine.

### NFR-02: File Size

- A 5 second GIF at default settings, 12 FPS, 100 percent scale, 800x600 area, must be under 5 MB.
- Frame deduplication must be applied: if two consecutive frames are identical, the second is dropped.

### NFR-03: Compatibility

- Must work on GNOME, Wayland, primary target.
- Must work on at least one more desktop in v1: KDE Plasma, Wayland, or Hyprland.
- Must use XDG Desktop Portal ScreenCast and PipeWire. No X11 `x11grab`, no screenshots via `grim` hacks.
- Must work on Flatpak and on native builds.

### NFR-04: Privacy

- No network requests. No telemetry. No crash reporting to a server.
- Screen content never leaves the machine unless the user exports and sends the GIF themselves.
- Blur annotations must actually obscure data in the exported GIF, not just in the preview.

### NFR-05: Accessibility

- All buttons must have accessible labels.
- The timeline must be keyboard navigable, arrow keys to move frame by frame.
- Color choices for annotations must not rely on color alone. Arrow and box also have shape, and text is always readable.

## 7. User Flow, Happy Path

1. Maya opens Bloop.
2. She clicks "Record Area" and drags a rectangle around the broken filter panel.
3. She clicks Record, clicks the filter, sees that nothing happens.
4. She clicks Stop. The editor opens.
5. She trims 1 second of dead time at the start.
6. She adds an arrow pointing to the filter button, and a text label: "Clicking does nothing."
7. She checks the size preview: 2.3 MB at 12 FPS. Good.
8. She clicks Copy, pastes the GIF into GitHub, and writes one sentence: "Filter does nothing on click, see GIF."
9. The developer watches the GIF, sees the bug in 3 seconds, and fixes it.

Total time: under 60 seconds. No upload, no link, no paragraph.

## 8. Edge Cases and Error Handling

- User denies portal permission: Show a clear message: "Bloop needs screen permission. Open Settings, Privacy, Screen Recording and allow Bloop." Do not show a blank black GIF.
- Window closes mid recording: Stop recording gracefully, keep the frames recorded so far, and tell the user what happened.
- No disk space on export: Show an error, do not create a corrupted partial GIF.
- Very large area, 4K monitor: Suggest 50 percent scale before export, with a one click "Make it smaller" button.
- Recording longer than 60 seconds: Auto stop and explain why.

## 9. Acceptance Criteria for v1

v1 is done when all of these are true:

- A new user can record, annotate, and export in under 60 seconds without reading docs
- GIFs are under 5 MB for 5 seconds at defaults
- Recording works on GNOME and KDE without black screen
- Blur actually hides data in the exported file
- All tests pass, clippy passes with zero warnings
- No MP4, no cloud, no AI, no account, the scope is clean
