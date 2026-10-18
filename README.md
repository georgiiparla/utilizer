# Utilizer

Windows CS2 practice player built with Rust, Axum, Askama, and HTMX.

The frontend is a minimal map prototype with Light/Dark themes and locally bundled
IBM Plex Sans and SVG maps, with spots on the left. Click a shaded zone to show
its spots. Until a zone is selected, the list stays empty. Map entry follows confirmed client log events
received while the app is running. Only the seven maps with bundled artwork are
selectable: Dust2, Mirage, Inferno, Nuke, Ancient, Cache and Anubis. Train, Overpass
and Vertigo are hidden until artwork is available. The theme selector remembers
your choice locally and switches the map artwork, controls and zone colors
together; Light is the initial default.
Nuke has upper-level artwork only. Rectangular zones group nearby starts and are clipped to
the bundled map outline, including cutouts. They are shortcuts to spots, not
grenade coverage or walkable-room boundaries.

Choose a map, then a zone. **Add spot** accepts a pasted
`setpos X Y Z;setang P Y R` command. Single-click selects immediately, locally;
double-click (or Enter) opens details with Save and Delete only.
The bottom-left dropdown switches **Mode: Saved** / **Mode: Captures**, with
separate points, zones and colors for the saved collection and pending buffer. Save a capture's details to keep it as a spot; Delete discards it.
**Add spot** sits top-center; **Help** beside Add spot opens the short setup guide. Playlists are not supported.

**Ctrl+S** is the only sync action. It exports the current map's saved spots and
selection to CS2's owned cfg directory. Captures must be saved before syncing.
Saving or deleting alone never writes game files. Sync protects unrelated files.
Notification chips sit bottom-center for three seconds with a shrinking duration
line; errors remain until interaction outside. Neither dialogs nor chips have a
close button: click outside to dismiss (Escape also closes dialogs). Hover previews and tooltip titles are omitted.
Sync writes exactly one `cfg.cfg` in CS2's cfg folder. Run `exec cfg` after
syncing to load the map state and bindings (loading alone does not teleport).
Use aliases such as `ut_7` to jump to slot 7. No per-spot cfg files are generated.
Slot numbers follow the current map's saved list, not IDs. Left/Right wrap through
spots; Up repeats the current spot; Down looks down at 89° while keeping its yaw.
F6 captures. Teleports send `[number/total] name` to team chat. Text is escaped;
stale aliases are cleared, and owned older configs are removed on explicit Sync.
Unrelated files and links remain untouched. Reload `exec cfg` after Sync.

The development Test feature and its input endpoint have been removed. Dev still
uses isolated data and a synthetic CS2 directory. On first dev startup, Mirage
gets sample spots and a capture buffer at different locations, including a busy zone and a long title
for layout checks. Click a painted zone to show its spots. Samples are editable
and are seeded only once; rebuilds preserve edits and deletions. Production is
never seeded.
The full feature inventory is in [the rebuild checklist](docs/frontend-rebuild.md).

Names, positions and URLs are bounded and validated on the server. Long list
names are ellipsized; dialog fields stay within the viewport. Open
dialogs are protected from polling refreshes, and stale edits from another window
are rejected. This first-version store starts fresh in `spots.sqlite3`; there are no migrations.
Old prototype databases are unused. Deleted spot IDs are never reused.

The local task runner is **just**; **watchexec** watches source files and restarts
the preview. Commands from the repository folder:

```text
# Live preview: opens the app window, watches files, and refreshes on save
just dev

# Keep startup and rebuilds from opening an app window
just dev --no-open-pwa

# Exercise console-code login explicitly
just dev --auth

# Stop the watcher and server from another terminal (or ask the agent to do it)
just stop

# Local verification: the same pipeline used by Windows CI
just check

# Individual tasks
just test
just lint
just build
```

On a fresh machine without just, run the Windows bootstrap once:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1
```

The bootstrap installs missing Rust 1.95.0, rustfmt, Clippy, Microsoft C++ build
tools with the Windows SDK, just 1.59.0, and watchexec 2.8.0. It skips installed
matching versions. Open a new terminal after first-time installation if the
commands are not yet on PATH. `just dev` and `just check` also run the prerequisite
check automatically. Windows may show its normal installer elevation prompt.
If installation requires a reboot, the bootstrap explains that and stops; run it
again after restarting. PowerShell handles Windows setup and installer checks;
the pipeline is defined in `justfile`, and watching uses watchexec's native
filesystem notifications and process supervision.

The preview runs at <http://127.0.0.1:47825> without login by default. Use
`just dev --auth` to test console-code login; enter the printed code once.
That session survives automatic rebuilds until you stop the development command.
Production always requires the console code. `--auth` is preview-only.
Dev and production open the PWA/app window by default after the server starts.
Dev also launches it after each successful rebuild/restart. The installed Utilizer
shortcut is preferred; otherwise Edge, then Chrome, is launched in app mode.
Use `just dev --no-open-pwa` or `utilizer --no-open-pwa` to only print the URL.
The flag stays in effect across dev rebuilds. `--open-pwa` remains accepted as an
explicit opt-in, and `--no-browser` is a compatibility alias for `--no-open-pwa`.
If neither browser is available, the server stays running and prints a manual-open
message. Combine with `--auth` in dev to test login. Ctrl+C stops the watcher and
its server. If the port is occupied, close the other instance first.
App-mode launch uses a 960 × 720 window. Installed PWAs apply that default once
(limited to the available screen), then leave resizing to the browser. The map
scales to the available window height.
You can also run `just stop` from another terminal. It stops this workspace's
marked watchexec process and its child server, including while rebuilding or
after a compilation error. Repeating it is safe. It does not kill arbitrary
processes using port 47825. To restart, run `just stop` and then `just dev`.
Closing the browser tab or installed app window only closes the page; use
`just stop` to stop the dev server.

To use a separate app window, open <http://127.0.0.1:47825> once in Edge or
Chrome and choose **Install Utilizer** from the browser's install icon or app
menu. Open the installed Utilizer shortcut for later sessions. Keep `just dev`
(or the production executable) running: the shortcut opens the UI, not the Rust
server. The installed window receives the same development refreshes.
The manifest and icons are bundled locally; no offline cache is installed.

CSS and JavaScript changes reload automatically, usually within one second.
Askama HTML templates and Rust changes rebuild the app, restart it, and refresh
the browser automatically. Cargo prints build duration in the terminal. Compilation
errors stay visible there; saving a correction retries the build.
Preview binaries use `target/dev-build/` so Windows executable locks do not
prevent running `just check` while the preview is open.

The preview stores spots and captures in `target/dev-preview/`, using
a synthetic CS2 directory inside it. Sync actions export only to that directory.
Sample positions are UI fixtures, not verified game lineups or protocol examples.
Data survives rebuilds and launcher restarts. To reset it, stop the preview and
delete only `target/dev-preview/`. Normal app startup still uses the real Steam
installation and normal local data.

`just check` runs formatting, Clippy and tests for normal and development builds,
an optimized production build, an executable smoke check, PowerShell syntax
checks, and existing isolated installer checks. It stops on failure and returns
a failing exit code. It does not install the app or publish anything.

CI uploads build artifacts for ordinary changes. Only a matching version tag
can trigger the existing draft-release job. Creating a version bump, tag, or
release requires an explicit release request; local development never does so.
