# Utilizer project guidance

Build a small Windows CS2 utility. Preserve the current milestone and the user's
accepted decisions; add architecture only when the next workflow needs it.

## Apply the installed skills

Read the applicable local `SKILL.md` before its first use in a task, and load only
the references needed for that change. State briefly which skill is being used.

- Rust implementation or review: `.agents/skills/rust-best-practices/SKILL.md`.
- HTMX interactions: `.agents/skills/htmx/SKILL.md`.
- New or revised visual design: `.agents/skills/frontend-design/SKILL.md`.

These are scoped guidance, not reasons to expand the task. Project decisions and
the user's instructions take precedence over reference examples. Check examples
against the installed crate/library versions before using them. Skill sources
and installed file hashes are recorded in `.agents/skill-sources.json`.

## Keep the core small

- Keep one Cargo package, a reusable library, and a thin executable. Preserve
  Rust 1.95 / edition 2024 and Cargo.lock; do not upgrade tools to copy an example.
- Use Tokio, notify, and bundled rusqlite as already implemented. Keep synchronous
  core operations and isolate blocking work from the async runtime. A dedicated
  database worker, pool, ORM, or new crate needs a concrete demonstrated need.
- Keep terminal messages simple. Do not add a logging framework, telemetry,
  observability stack, or persistent diagnostic logs as a default.
- Keep `utilizer-ruby/` as local reference material for features and algorithms.
  Leave its files unchanged and untracked; exclude it only through
  `.git/info/exclude`, never the tracked `.gitignore`. Do not add it to commits.
  Tests and development checks use temporary fixtures and must not modify the
  real CS2 installation.
- Preserve callback-only signaling, incremental bounded log reads, two-second
  recovery, and explicit capture timeouts. Synthetic capture fixtures do not
  establish the actual CS2 marker/getpos output format.
- Startup must not export cfgs or change game files. Keep export explicitly
  requested, confined to owned output, and protected against unrelated files.

## Browser stage

- Use Axum + Askama + HTMX, server-rendered HTML, and ordinary CSS. Keep the
  existing core independently usable. Do not introduce React, a SPA, or a
  frontend build system by default.
- Use the selected compact music-player interface: map/playlist selection,
  numbered spots, and bottom selected-spot controls. Fit the small 4:3 Steam
  overlay. Prioritize clear actions, keyboard focus, and readable spacing.
- Honor the library version selected for the project; do not mix HTMX 2 and 4
  APIs just because the skill documents both. Escape user content in templates.
- Bind to `127.0.0.1:47825` and fail clearly if occupied. Preserve the planned
  console-code login and server-run sessions. Add the smallest browser slice
  only when requested; skill installation does not authorize starting it.
- GitHub Actions currently performs CI checks and uploads build artifacts only.
  Do not create tags or GitHub Releases, including drafts, before the UI exists
  and the user requests a release.

## Verification

Test behavior and meaningful failure boundaries with the existing Rust suites.
Do not add browser tests, coverage targets, snapshot frameworks, mocks, fuzzers,
or benchmarks just to follow a reference. The native-notification integration
test must still work with recovery polling disabled and a bounded deadline.

For Rust changes, run the applicable existing checks and report actual results:

```powershell
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked --release
```

Reuse valid results when no relevant files changed. Documentation or skill-only
changes need installation/reference checks, not new application tests.
