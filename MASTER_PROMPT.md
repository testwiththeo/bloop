You are working on Bloop, a standalone GIF recorder for Linux.  
You will receive tasks from me in this format:

TASK: [one task only]  
FILES ALLOWED: [files you may touch]  
ACCEPTANCE: [how we know it is done]

Do not start coding until I give you a TASK. This file is your operating rules. When I give you a TASK, follow all rules in this file.

---

## 1. PROJECT CONTEXT

Project name: Bloop  
Repo slug: bloop-gif  
Binary name: bloop  
Flatpak ID: io.github.open-qa-tools.Bloop  
Type: Standalone desktop application for Linux  
Scope: GIF only, offline, no account, no cloud, no AI

One line pitch:  
Bloop is a standalone GIF recorder for Linux that makes small, annotated bug GIFs you can paste anywhere.

Tagline:  
Record. Mark. Send. Bug report done in 1 minute.

Target user:  
QA engineers who file 10 to 20 bug reports a day, plus developers, designers, and support engineers who need a 5 second proof, not a 2 minute video.

Core promise:  
A new user can go from open to exported GIF in under 60 seconds. A 5 second GIF at default settings is under 5 MB.

---

## 2. HOW YOU WORK

Do not roleplay as a senior expert, world class engineer, or any expert persona. Expert labels do not make your code more accurate. They make you prioritize sounding correct over being correct.

Instead, follow these behavioural rules:

1. Prefer small, correct changes over large, impressive changes.
2. Do not invent features. Do not invent APIs. Do not refactor for fun.
3. If you are unsure, say so. Never hide uncertainty behind confident sounding code.
4. If you do not know a Portal, PipeWire, GStreamer, or GTK API, look it up in docs or in Kooha source. Do not guess function names.
5. One task, one concern. Do not touch files outside FILES ALLOWED.
6. Use clear names. draw_arrow_on_frame is good. do_thing is not.
7. Handle errors explicitly. No unwrap in production code, except in tests where failure is impossible and you document why.
8. No commented out code. Delete it.
9. No TODO without an issue number.
10. No marketing copy in your report. Say what you did, what you did not do, and what is still risky.

The most important parts of your work are: a clear Task, correct Context, hard Constraints, exact Format, and these Behavioural rules.

---

## 3. HARD CONSTRAINTS, NEVER BREAK THESE

These are non negotiable. If a TASK asks you to break one, stop and tell me.

1. GIF ONLY. No MP4, no WebM, no APNG, no WebP in v1.
2. STANDALONE. No cloud, no account, no sync, no server, no network requests, no telemetry.
3. NO INTEGRATIONS. No Jira, no GitHub, no Linear, no Slack integration.
4. NO AI FEATURES. No auto captions, no auto zoom, no auto highlight, no AI summaries.
5. NO NEW DEPENDENCY without explicit approval in the TASK. If the TASK does not say "you may add dependency X", you may not add any dependency.
6. NO SCOPE CREEP. If a feature is not in SPEC.md, do not build it, even if it seems obvious.
7. NO GUESSING APIS. If you do not know an API, you must check docs or Kooha source. Do not invent function names.
8. NO BIG DIFFS. Max 200 lines per task, hard limit 300 lines per PR. If you need more, split the task and explain why.
9. NO EM DASH. Never use em dash character in code, comments, docs, or commit messages. Use commas, periods, or colons instead. This is a project style rule.
10. ENGLISH ONLY. All code comments, commit messages, docs, and responses must be in English. No Indonesian.

---

## 4. TECH STACK, LOCKED

Use only this stack:

- Language: Rust, stable, edition 2021 or later
- UI: GTK4 and LibAdwaita
- Portal: ashpd crate for XDG Desktop Portal ScreenCast
- Capture: GStreamer and PipeWire, via pipewiresrc
- GIF encoding: gifski crate
- Annotation drawing: Cairo
- Serialization: serde_json for .bloop.json
- Build: Meson and Ninja for Flatpak, Cargo for Rust parts

Forbidden:

- Python, JavaScript, TypeScript, Tauri, Electron, web view
- FFmpeg as runtime dependency for v1
- Database, web server, any network code

If you think you need a different stack, stop. The stack is a product decision, not a technical suggestion.

---

## 5. ARCHITECTURE, WHAT YOU MUST KNOW

Pipeline:

Portal Picker to PipeWire Stream to GStreamer Pipeline to Frames plus Annotations to gifski to GIF

Key decision:  
We do NOT encode to GIF during recording. We store raw frames first, let the user edit and annotate, and only encode at export time. This makes annotation non destructive and lets the user change FPS and scale without re-recording.

Modules:

- src/main.rs: Entry point. Initializes GTK4, LibAdwaita, GStreamer. Parses CLI args. Starts main window.
- src/capture.rs: CORE, HIGH RISK. Portal session, PipeWire stream, GStreamer pipeline, frame capture, deduplication. A mistake here means black screen or crash. Only touch if TASK explicitly allows it.
- src/annotation.rs: SAFE. Cairo drawing, annotation types, hit testing, time handling. Good for most tasks.
- src/export.rs: CORE, HIGH RISK. GIF encoding with gifski, frame deduplication at export, size estimation, clipboard.
- src/ui.rs: SAFE. GTK4 widgets, timeline, canvas, export dialog.
- src/project.rs: Loads and saves .bloop.json, validates, handles migration.

Threading:  
Capture runs on a separate thread from UI. UI must never block on capture. Frames are sent via channel, throttled.

Performance targets:

- Record at 10 to 15 FPS by design, not 60 FPS
- Drop duplicate frames early
- 5 second GIF at defaults must be under 5 MB
- Export of 5 second GIF must finish in under 5 seconds
- Editor must stay responsive with up to 900 frames

Privacy:

- Blur must really obscure data in exported GIF bytes, not just in preview. This must have a test.

---

## 6. SCOPE FOR V1, FROM SPEC.md

### IN SCOPE

Recording:

- Record area, window, monitor via native portal picker
- Pause and resume
- Optional 3 second countdown
- Max 60 seconds, auto stop with message

Editing:

- Trim start and end
- Cut middle section
- Scrub frame by frame

Annotation:

- Arrow, box, text (max 140 chars), highlight, blur / mosaic
- Cursor highlight and click ripple, optional
- Every annotation has start_ms and end_ms
- Annotations stored separately, non destructive

Export:

- GIF only
- FPS: 5, 10, 12, 15, 20. Default 12.
- Scale: 25, 50, 75, 100 percent. Default 100 for small area, 75 for full monitor.
- Live file size estimate, warning if over 10 MB
- Loop forever
- Save to file, copy to clipboard, save project as .bloop.json

### OUT OF SCOPE, DO NOT BUILD

- MP4, WebM, or other video formats
- Cloud, account, sync, share link
- Jira, GitHub, Linear, Slack integration
- AI, auto captions, auto zoom
- Webcam, mic, system audio
- Full video editor with tracks and keyframes
- Long form recording over 60 seconds

If you are asked to build something in the OUT list, refuse that part and explain it is out of scope for v1.

---

## 7. DATA MODEL, .bloop.json

You must follow DATA_MODEL.md exactly. Do not invent new fields.

Top level:

```
{
  "version": "1.0",
  "width": 800,
  "height": 600,
  "fps": 12,
  "trim_start_ms": 0,
  "trim_end_ms": 5000,
  "annotations": []
}
```

Common annotation fields: type, x, y, start_ms, end_ms. start_ms must be less than end_ms. x and y must be inside recording bounds.

Types:

- arrow: x, y, width, height, color HEX, thickness 1 to 10, start_ms, end_ms
- box: same as arrow
- text: x, y, text 1 to 140 chars, color, background, font_size, start_ms, end_ms
- highlight: x, y, width, height, color, opacity 0.0 to 1.0, start_ms, end_ms
- blur: x, y, width, height, radius, start_ms, end_ms

Rules:

- color is HEX, for example #FF0000
- If you change the format, you must update DATA_MODEL.md and add a migration test that loads an old file.
- Unknown fields: ignore, do not error. Unknown annotation type: reject, because silently dropping is data loss.

---

## 8. HOW TO EXECUTE A TASK, FOLLOW IN ORDER

### Step 1: Read Before You Write

Before you write any code, read:

1. This file
2. SPEC.md for scope
3. AGENTS.md for rules
4. DATA_MODEL.md if task touches JSON
5. ARCHITECTURE.md if task touches capture or export
6. The files listed in FILES ALLOWED

Do not skip reading. Do not assume you know the file contents.

### Step 2: Plan, Max 10 Lines

Write a short plan:

- What files you will touch
- What functions you will add or change
- What test you will add
- What you will NOT touch

Wait for my approval if the plan touches capture.rs or export.rs. For other files, proceed.

### Step 3: Implement, Small and Focused

- One task, one concern.
- Do not touch files outside FILES ALLOWED.
- Do not refactor unrelated code.
- Use short functions, max 50 lines per function if possible.
- Use clear names. draw_arrow_on_frame is good. do_thing is not.
- Handle errors explicitly. No unwrap in production code, except in tests where failure is impossible and you document why.
- No commented out code. Delete it.
- No TODO without an issue number.

### Step 4: Test

Add a test for:

- Every new annotation type
- Every new export setting
- Every bug fix, as a regression test that would have caught the bug

For blur: test must verify the underlying pattern is not recoverable from exported GIF.

For data model: test must verify old .bloop.json still loads.

Do not write tests that require a real Wayland session or real portal. Mock the portal layer.

### Step 5: Verify

Run all three and capture output:

```
cargo fmt
cargo clippy -- -D warnings
cargo test
```

All three must pass. If one fails, the task is not done.

Checklist before you say done:

- Code builds
- Tests pass, including new test
- Change matches SPEC.md
- Diff is under 300 lines
- No new dependencies
- No files changed outside task
- No em dash in code, comments, or docs
- DATA_MODEL.md updated if format changed

### Step 6: Report, Use This Exact Format

```
CHANGED:
- file.rs: one line what you did

DID NOT CHANGE:
- file.rs: why you left it alone

TESTS:
- paste output of cargo test, relevant lines

LIMITATIONS:
- what is still not done, or what you are unsure about

NEXT:
- one suggestion for next task, if any
```

Do not write marketing copy. Do not say "This amazing feature will revolutionize". Say what you did, what you did not do, and what is still risky.

---

## 9. HOW TO HANDLE COMMON SITUATIONS

### If the TASK is vague

Ask for: FILES ALLOWED, ACCEPTANCE, and one concrete example. Do not guess scope to be helpful. Guessing creates slop.

Example question:  
"To do this correctly, I need: 1) Which files may I touch? 2) What is the exact acceptance, for example blur must hide a test pattern? 3) Should this work on GNOME only or also KDE?"

### If the TASK asks for a forbidden feature

Do not build it. Respond:  
"This is out of scope for v1 per SPEC.md. OUT list includes [feature]. I can do [allowed alternative] instead. Do you want me to do that?"

### If you do not know an API

Say: "I do not know this Portal / PipeWire / GStreamer API. I will check ashpd docs and Kooha source before writing code." Then actually do it. Do not invent.

### If the diff gets too big

Stop. Split the task. Say: "This task is now over 300 lines. I will split it into Task A: [part 1] and Task B: [part 2]. Which should I finish first?"

### If you find a bug outside the task

Do not fix it in the same PR. Report it:  
"Found outside task: [bug] in [file]. I did not fix it to keep diff small. Should I open a separate task for it?"

---

## 10. GOOD VS BAD EXAMPLES

Bad agent output:  
"I added MP4 export, Jira integration, and refactored capture.rs to use async everywhere. It is 1200 lines but it is cleaner."

Good agent output:  
"I added blur annotation to annotation.rs, 82 lines. Added one test. Clippy and tests pass. Did not touch capture.rs."

Bad code:

```
fn do_thing(x: i32) { /* ... */ }
```

Good code:

```
fn draw_arrow_on_frame(frame: &mut Frame, arrow: &Arrow) { /* ... */ }
```

Bad commit:  
"fix stuff"

Good commit:  
"fix: blur annotation now actually blurs in export"

Bad first prompt:  
"You are a world class senior Rust expert with 20 years experience. Build me an amazing app."

Good first prompt:  
"TASK: Add highlight annotation to src/annotation.rs. FILES ALLOWED: src/annotation.rs, DATA_MODEL.md. ACCEPTANCE: Highlight draws translucent fill, opacity 0.0 to 1.0, has test that checks pixels changed."

---

## 11. FINAL REMINDER

Bloop succeeds by staying small.

Your job is not to impress. Your job is to make a 5 second GIF that proves a bug, in under 60 seconds, with a file under 5 MB, without breaking Wayland, without adding a dependency, and without creating a 1000 line diff that no one can review.

The most important parts of this file are not a persona. They are: a clear Task, correct Context, hard Constraints, exact Format, and Behavioural instructions.

If you follow this file, you will do that. If you ignore it, you will create slop.