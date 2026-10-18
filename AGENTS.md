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

- Active map sync writes exactly one cfg-root `cfg.cfg`. Load with `exec cfg`;
  `ut_7` jumps via a CS2 alias. Never generate numbered per-spot cfg files.
  Keep four arrows (Down = pitch 89, current spot yaw), F6 capture, and Ruby's
  `[number/total] name` team chat. Preserve text escaping, alias length bounds,
  stale alias resets and preflight protection of unrelated files/reparse points.
  Only explicit Sync may replace/delete recognized owned generated cfg files.

## Browser stage

- Use Axum + Askama + HTMX, server-rendered HTML, and ordinary CSS. Keep the
  existing core independently usable. Do not introduce React, a SPA, or a
  frontend build system by default.
- Current milestone: minimal map/zone UI with map-owned spot CRUD, a per-map
  bottom-left Mode: Saved / Mode: Captures dropdown, Ctrl+S sync and Light/Dark themes. No playlists or migrations.
  Add spot is centered at the top; no Sync button, Save + sync, copy/order controls, hover
  previews or tooltip titles. Internal spot/zone actions use buttons, not links,
  so PWA hover never shows their URLs. Click selects immediately in the DOM without a
  request or delay; double-click/Enter opens Save/Delete details. Help sits
  beside Add spot at top-center; Theme stays top-right. Modals have no close button: click the backdrop
  or use Escape. Notices are bottom-center with no close button: timed with a
  duration line, or important until interacting outside. Preserve bounded inputs, ellipsized rows,
  polling-safe drafts, stale-edit checks and non-reused IDs. No Test panel or
  synthetic-input HTTP endpoint. Dev seeds editable Mirage spots and a differently positioned buffer once;
  preserve edits/deletions on rebuild and never seed production.
  Offer only the seven maps with local artwork; omit Train, Overpass and Vertigo.
  Preserve `docs/frontend-rebuild.md` and consult the Rust/Ruby references before
  claiming feature parity. Keep the interface usable in the small Steam overlay.
- This is a utility: no slogans, promotional copy, decorative album artwork,
  or landing-page sections. Use the existing gold icon. Keep setup instructions
  in expandable help and label actions by what they actually do. Installation
  must show its stages and download progress; keep console output concise.
- Honor the library version selected for the project; do not mix HTMX 2 and 4
  APIs just because the skill documents both. Escape user content in templates.
- Bind to `127.0.0.1:47825` and fail clearly if occupied. Preserve the planned
  console-code login and server-run sessions. Add the smallest browser slice
  only when requested; skill installation does not authorize starting it.
- GitHub Actions runs Windows CI and prepares versioned releases. The user has
  authorized the first UI release and making the repository public for the
  PowerShell installer. Future releases still need a user request.

## Verification

Use `just dev` for frontend iteration. It automatically prepares missing
tools and runs an isolated live preview; do not use real CS2 or production data
for UI checks. Preview skips login by default; use `just dev --auth` to test
console-code authentication. Production always requires login. Default startup opens the PWA/app window, including after successful dev
rebuilds. `--no-open-pwa` disables launching throughout the dev run; keep
`--no-browser` as a compatibility alias. Prefer the installed shortcut or use
Edge/Chrome app mode; never fall back to a browser tab. The PWA can be installed from Edge/Chrome for a
separate window. File watching and process restarts belong to watchexec; do not reimplement them in
PowerShell. Keep Rust 1.95.0. `scripts/setup.ps1` bootstraps missing Windows tools
and installs pinned just/watchexec versions when needed.

Manage preview lifetime with `just dev` and `just stop`. Reuse a
running preview during UI iteration. When asked to close it, run `just stop`;
this stops only the watcher marked for this workspace and its process tree,
including during a failed build. Never find and kill an arbitrary port owner.
To restart, run `just stop` followed by `just dev`. Leave unrelated
apps and other workspaces alone. Closing a browser tab does not stop the server.

Run applicable local checks automatically; `just check` prepares missing
tools and runs the same checks as Windows CI, including the dev feature and
installer checks. Do not require the user to install tools or run checks manually
when the agent can do it. Reuse valid check results where appropriate.

Ordinary development does not authorize version bumps, release tags, release
drafts, or publication. Each future release needs an explicit user request.

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
