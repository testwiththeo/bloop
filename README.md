# Bloop

**Record. Mark. Send. Bug report done in 1 minute.**

Bloop is a standalone GIF recorder for Linux that makes small, annotated bug GIFs you can paste anywhere.

- **GIF only.** No MP4, no WebM, no cloud, no account, no AI. One job, done well.
- **Annotation built in.** Draw an arrow, box, text, highlight, or blur right on the GIF.
- **Small files.** A 5 second GIF at default settings targets under 5 MB.
- **Fast flow.** Record to export in under 60 seconds.

![Bloop demo](./docs/demo.gif)
*Demo GIF made with Bloop itself. Replace this file with a real recording once you have a build.*

## Why Bloop Exists

Text alone is ambiguous. "The button does not work" can mean five different things. Video is too heavy. A 30 second MP4 of a full monitor is tens of MB, needs an upload, needs a player, and forces the viewer to scrub to find the three seconds that matter.

GIFs solve both problems. They autoplay inline in GitHub, Jira, Slack, Discord, and email. They loop. They are self contained.

Peek was the classic GIF recorder. It is now effectively unmaintained, has no annotation, and struggles on modern Wayland.

Kooha is excellent, but it is a general video recorder. It is not built for tiny, annotated bug GIFs with a size preview.

Bloop is the missing tool: record, annotate, export a small GIF, paste it, done.

## Why Not X?

| Tool | What it does | Why Bloop is different |
| --- | --- | --- |
| Peek | Simple GIF recorder | Unmaintained, no annotation, weak on Wayland |
| Kooha | General video recorder | Great for video, not for small annotated GIFs |
| OBS | Full streaming studio | Too heavy for a 5 second bug GIF |
| Bloop | GIF only bug GIFs | Record, annotate, size preview, export in 1 minute |

## Quick Start

Install with Flatpak:

```bash
flatpak install flathub io.github.open-qa-tools.Bloop
```

Requirements: Wayland, XDG Desktop Portal, PipeWire. Works on GNOME and KDE.

Then:

1. Open Bloop
2. Choose Area, Window, or Monitor
3. Click Record, reproduce the bug
4. Click Stop
5. Trim dead time, add an arrow and a short label
6. Check the size preview, lower FPS or scale if needed
7. Export and paste the GIF into your bug report

## Features

### Recording

- Record a selected area, a window, or a monitor
- Pause and resume without splitting the GIF
- Optional 3 second countdown
- Max 60 seconds in v1, on purpose. Bloop is for short proofs.

### Editing

- Trim start and end with a visual timeline
- Cut out a middle section
- Scrub frame by frame

### Annotation

- Arrow, box, text, highlight, blur
- Every annotation has a start time and end time
- Annotations are non destructive. They live in `.bloop.json` until export.
- Blur really hides data in the exported GIF, not just in preview.

### Export

- GIF only
- Controls: FPS 5, 10, 12, 15, 20. Scale 25, 50, 75, 100 percent.
- Live file size estimate before export
- Warning if estimate is over 10 MB
- Save to file or copy to clipboard

## What Bloop Does Not Do

- No MP4, WebM, APNG, or WebP in v1
- No cloud, sync, account, or share link
- No Jira, GitHub, or Slack integration
- No AI captions, auto zoom, or auto highlight
- No webcam, mic, or system audio
- No full video timeline editor

If you want those, use another tool. Bloop is for the opposite problem: the smallest artifact that proves the bug.

## Who Is This For

- **QA engineers** who file 10 to 20 bugs a day and need each one to be unambiguous
- **Developers** who want to show a 4 second UI change in a PR
- **Designers** who need to point out a 2 pixel misalignment
- **Support engineers** who need to show a repro and blur customer data first

## Docs

| Doc | What it answers |
| --- | --- |
| SPEC.md | What is in and out for v1 |
| ARCHITECTURE.md | How capture, annotate, and export work |
| DATA_MODEL.md | Format of `.bloop.json` |
| CONTRIBUTING.md | How to run and contribute |
| AGENTS.md | Rules for AI agents |

## Contributing

Bloop is intentionally small, so it is a good first Rust and GTK project. See CONTRIBUTING.md. Start in `src/annotation.rs` or `src/ui.rs` if you are new.

## License

MIT. See LICENSE.
