# CONTRIBUTING.md, How to Help with Bloop

## Welcome

Thank you for considering a contribution to Bloop. Bloop is intentionally small, and we intend to keep it that way. That makes it a good project to contribute to: you can understand the whole codebase in an afternoon, and your contribution will actually be visible in the product, not buried in a 10000 file monorepo.

This document explains how to set up your environment, how to pick a task, how to submit a PR, and what we expect from reviewers. Please read it before you start coding.

## 1. Code of Conduct, Short Version

Be kind. Be clear. Help newcomers. Assume good intent.

We are building a tool for QA engineers, who are often the most ignored people in a software team. Let us not reproduce that dynamic in our own community. If someone asks a basic question, answer it. If someone PR is imperfect, explain how to improve it instead of dismissing it.

For the full text, we use the Contributor Covenant. If you experience harassment or unkind behavior, contact the maintainers privately.

## 2. Setting Up Your Environment

### Prerequisites

- Linux with Wayland, GNOME is the primary target, KDE also works
- Rust stable, via `rustup`
- Meson and Ninja
- Flatpak, for testing the Flatpak build
- PipeWire and XDG Desktop Portal installed, they are present on most modern distros

### Clone and Build

```bash
git clone https://github.com/open-qa-tools/bloop-gif.git
cd bloop-gif
meson setup build
ninja -C build
```

### Run

```bash
flatpak run io.github.open-qa-tools.Bloop
```

Or, for a native development build:

```bash
cargo run
```

If the portal picker does not appear, check that `xdg-desktop-portal` and `xdg-desktop-portal-gnome` or `-kde` are installed. If you get a black screen, see ARCHITECTURE.md for the capture pipeline and check whether PipeWire is running: `systemctl --user status pipewire`.

## 3. How to Pick a Task

### Good First Issues

Look for issues labeled `good first issue`. These are tasks that are:

- Small, under 200 lines
- Isolated, in `src/annotation.rs` or `src/ui.rs`
- Well specified, with acceptance criteria in the issue

Examples of good first issues:

- Add a new annotation color preset
- Fix the timeline scrubbing so it snaps to frames
- Add a keyboard shortcut for "Add Text", T
- Fix a typo in the export dialog

### What to Avoid If You Are New

- Do not start in `src/capture.rs`. Portal plus PipeWire is subtle and hard to debug without Wayland experience.
- Do not start with a feature that is not in SPEC.md. If it is not in SPEC, it needs a discussion first.
- Do not start with a big refactor. Earn context by fixing small bugs first.

### Proposing a New Feature

If you have an idea for a feature that is not in SPEC.md:

1. Open an issue with the title "[Proposal] Your idea"
2. Explain the use case: who needs it, and why the current tool cannot do it
3. Explain why it fits Bloop philosophy, GIF only, standalone, small files
4. Wait for maintainer feedback before coding

This saves you from building something that will be rejected for scope reasons.

## 4. Development Workflow

### Branching

- `main` is always releasable. Do not push directly to `main`.
- Create a branch from `main`: `git checkout -b fix/blur-export`
- Use prefixes: `feat/` for features, `fix/` for bugs, `docs/` for docs, `test/` for tests

### Commits

- Write clear commit messages: `fix: blur annotation now actually blurs in export` is good. `fix stuff` is not.
- One logical change per commit. Do not mix a bug fix and a refactor in one commit.

### PR Rules

Every PR must:

1. Be one task. If you fixed two bugs, make two PRs.
2. Be under 300 lines of diff. If it is larger, explain why in the description and consider splitting it.
3. Include a test for the change. For a bug fix, include a regression test that would have caught the bug.
4. Pass all checks:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```
5. Update docs if you changed behavior. If you changed the JSON format, update DATA_MODEL.md. If you changed scope, update SPEC.md.
6. Have a clear description: what you changed, why, and how you tested it. Include a GIF of the behavior if it is a UI change. Bloop can record itself, which is a nice dogfooding opportunity.

### Review Process

- A maintainer will review within a few days. Be patient, this is a volunteer project.
- Review comments are not personal. They are about keeping the codebase small and consistent.
- If you disagree with a review comment, explain your reasoning. A good discussion is welcome.
- After approval, a maintainer will merge. Do not merge your own PR unless you are a maintainer.

## 5. Testing

### What to Test

- Annotation: Every annotation type must have a test that draws it on a blank frame and checks that pixels changed in the expected region.
- Export: Export must have a test that exports a 1 second GIF and checks that the file exists, is non empty, and is a valid GIF.
- Data Model: Every migration must have a test that loads an old `.bloop.json` file and asserts it still loads.
- Blur: Blur must have a test that verifies the underlying pattern is not recoverable from the exported GIF.

### What Not to Test

- Do not write tests that require a real Wayland session or a real portal. Those are integration tests that cannot run in CI. Mock the portal layer instead.

## 6. What We Want vs What We Do Not Want

### We Want

- Bug fixes, especially for capture on KDE and Hyprland
- Tests, especially for export and blur
- Small UI polish that makes the 60 second flow faster
- Translations
- Better GIF optimization, smaller files, same quality
- Documentation improvements. If you were confused by a doc, fix it.

### We Do Not Want

- MP4 or WebM support in v1
- Cloud, account, sync, or AI features
- Jira or GitHub integration
- Big refactors without a prior issue
- New dependencies without justification

## 7. Getting Help

- Open an issue with the label `question` if you are stuck.
- Mention what you tried, what you expected, and what happened. Include logs if you have them.
- Do not DM maintainers for support. Use issues so everyone can learn from the answer.

Thank you again. The best contribution you can make is not a huge feature. It is a small, well tested fix that makes Bloop a little more reliable for the next QA engineer who is filing their 15th bug of the day.
