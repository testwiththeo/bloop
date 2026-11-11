# AGENTS.md

This file is for AI agents and human contributors. Follow it strictly. If a rule conflicts with speed, the rule wins.

## Stack Is Locked

Use only this stack. Do not propose alternatives unless the maintainer explicitly asks for a re-evaluation.

- Language: Rust, stable, edition 2021 or later
- UI: GTK4 and LibAdwaita
- Portal: `ashpd` crate for XDG Desktop Portal ScreenCast
- Capture: GStreamer and PipeWire, via `pipewiresrc`
- GIF encoding: `gifski` crate
- Annotation drawing: Cairo
- Serialization: `serde_json` for `.bloop.json`
- Build: Meson and Ninja for Flatpak, Cargo for Rust parts

Do not add a new dependency without maintainer approval, with a justification in the PR description that explains why the existing stack cannot do it.

Do not introduce Python, JavaScript, TypeScript, Tauri, Electron, or any web view.

Do not introduce FFmpeg as a runtime dependency for v1. `gifski` is the encoder.

Do not introduce a database, a web server, or any network code. Bloop is offline by design.

## Scope Is Locked by SPEC.md

SPEC.md is the contract. If a feature is not in the In Scope list, it does not exist, no matter how reasonable it sounds.

Examples that sound reasonable but are forbidden in v1:

- "Let us also export MP4, it is just one more button." No. GIF only is the product.
- "Let us add Jira integration, QA people use Jira." No. Bloop is standalone. You export a GIF and paste it yourself.
- "Let us add AI auto captions." No. No AI in v1.
- "Let us add a cloud share link." No. No cloud, no account.

If you believe a feature should be in scope, open an issue and discuss it. Do not build it in a PR that was supposed to be about something else.

## Task Discipline

### One Task, One PR, One Concern

- Do exactly one task at a time.
- Do not bundle a bug fix, a refactor, and a new feature into one PR.
- Do not touch files outside the task scope. If the task says fix blur in `annotation.rs`, do not also clean up `capture.rs`.
- Keep the diff small: target under 200 lines per task, hard limit 300 lines per PR. If you need more, split the work and explain why in the PR description.

Why this matters: A 300 line PR can be reviewed in 10 minutes. A 1500 line PR will sit unreviewed for a week and then get merged without real review, which is how bugs get in.

### Never Guess an API

Wayland portals, PipeWire, and GStreamer are unforgiving. Do not invent function names, signal names, or property names from memory.

Before you use an API you are not sure about:

1. Read the `ashpd` docs, or
2. Read Kooha source code, it is the reference implementation for ScreenCast on GNOME, or
3. Read the GStreamer docs for the specific element you are using.

If you cannot find the API, say so in the PR and ask for help. Guessing and hoping it compiles is not a strategy. It often does compile and then fails at runtime with a black screen.

### No Drive-By Refactors

Do not rename variables, reformat unrelated code, or modernize a file that is not in the task. Refactors are legitimate work, but they must be their own PR with their own justification. Mixing a refactor with a bug fix makes both harder to review and harder to revert.

## File Ownership and Risk Levels

Not all files are equal. Some files can break the whole app if you get them wrong.

### Core, High Risk

- `src/capture.rs`: Portal negotiation, PipeWire stream setup, GStreamer pipeline. A mistake here means black screen, frozen frames, or a crash on KDE. Only touch this if you understand Wayland session handling.
- `src/export.rs`: GIF encoding, frame deduplication, palette generation. A mistake here means huge files or corrupted GIFs.

### Safe, Good for Beginners

- `src/annotation.rs`: Cairo drawing, annotation types, hit testing. This is where most good first issues live. It is isolated and easy to test.
- `src/ui.rs`: GTK4 widgets, dialogs, timeline UI. Safe to experiment, but keep the UI simple and consistent with LibAdwaita.

If you are new to the project, start in the Safe files. Earn trust in the Core files by reviewing them first.

## Code Quality Standards

- Use short functions. If a function is longer than 50 lines, consider splitting it.
- Use clear names. `draw_arrow_on_frame` is good. `do_thing` is not.
- Do not add TODO without an issue number. A bare TODO is a promise nobody will keep.
- Do not leave commented out code. Delete it. Git remembers it if you need it back.
- Handle errors explicitly. Do not use `.unwrap()` in production code paths, except in tests or in code where failure is truly impossible and you have documented why.
- Write a test for every new annotation type, every new export setting, and every bug fix. A bug fix without a regression test is a bug that will come back.

## Verification, Mandatory Before Done

Before you claim a task is done, run all three commands and include their output in your PR or task summary:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

- `cargo fmt` must produce no diff.
- `cargo clippy -- -D warnings` must pass with zero warnings.
- `cargo test` must pass. All tests, not just the one you added.

Done also means:

- The change matches SPEC.md
- The diff is under 300 lines
- No new dependencies were added without approval
- No files outside the task scope were changed
- DATA_MODEL.md was updated if you changed the JSON format, and you added a migration test for the old format

If any of these are not true, the task is not done, even if the code compiles.

## Communication Style for AI Agents

When you finish a task, report it in this format:

- What you changed, file names, one line each
- What you did NOT change and why
- Test output, paste the relevant lines
- Known limitations or follow ups

Do not write marketing copy. Do not write "This amazing new feature will revolutionize your workflow." Write "Added blur annotation. Test passes. File size preview still needs a fix for 4K."

Be honest about uncertainty. If you are not sure whether a portal API behaves the same on KDE as on GNOME, say so. The maintainer would rather know about a risk than discover it after merging.

## Examples of Good vs Bad AI Behavior

Bad: "I added MP4 export, Jira integration, and refactored capture.rs to use async everywhere. It is 1200 lines but it is cleaner."

Good: "I added blur annotation to annotation.rs, 82 lines. Added one test. Clippy and tests pass. Did not touch capture.rs."

The difference is not talent. It is discipline. Follow this file and you will be in the second category.
