# Frontend rebuild checklist

The current interface keeps the map, flat spot list, selectors, Add spot and
a Saved/Captures mode dropdown beside Add spot. Ctrl+S is the only sync action. Light/Dark preferences and matching local map artwork are preserved. Only
Dust2, Mirage, Inferno, Nuke, Ancient, Cache and Anubis are selectable.
The production executable embeds all runtime HTML, CSS, JavaScript, font, map and
icon assets. Normal operation needs only the local server at 127.0.0.1:47825,
an installed Edge/Chrome browser and the local CS2 installation; no internet
connection or CDN is required. CSP restricts asset loads and requests to self.
Credit/reference links use the internet only if opened; installation downloads
and development tool bootstrapping are separate from offline application use.
The app window defaults to 760×760. Installed PWAs apply the square default once,
clamped to the available screen size, then retain the user's later resizing.

The transparent compass emblem has a clean vector source in `assets/logo.svg`,
used directly in the app. Its orange/gold geometry fills the canvas with a small
clear margin, without the raster source's stray pixels. PWA PNGs and the Windows
ICO are rasterized from this SVG. The ICO contains 16, 20, 24, 32, 40, 48, 64, 96,
128 and 256 pixel frames and is embedded through the Windows resource compiler.
Icon URLs are versioned to refresh browser caches; already installed PWAs and
pinned Windows shortcuts may retain their cached icon until refreshed or reinstalled.
On Windows, the same emblem appears in the system tray. Its native menu has two
items: a plain status (Online, Retrying or Starting), the current login code and Exit.
The code remains valid for this process run and changes on restart. It appears only
in the menu, not the tooltip; previews without authentication show Login not required.
Anyone with access to the desktop tray can read it. Online means Utilizer
is running, including while waiting for CS2's log. Status also appears in the
tooltip. Left-click reuses the existing PWA/app window, restoring it if minimized;
right-click opens the native menu. The launcher checks the exact current or legacy
Utilizer title, Chromium app-window class and Edge/Chrome executable before focusing
a window. Regular browser windows are excluded. A new window is launched only when
none matches; startup and tray launches share a gate and a ten-second opening guard.
If Windows denies foreground focus, the existing window's taskbar button flashes.
Exit uses the same graceful server/reader shutdown as Ctrl+C; the tray
icon is removed on shutdown. The browser/PWA window remains browser-owned and
reloads on a confirmed disconnect, normally within a second, to show the browser's
unavailable-page screen. Connection checks continue while dialogs are open;
timeouts alone do not trigger a reload.

## Reference checkpoints

- Original Rust player: `435520b61113759a9534d61fa9a5136cfe2ac9f2`, including
  `src/web.rs`, templates and assets.
- Read-only local reference: `utilizer-ruby/lib/utilizer/web_app.rb`, its
  templates, playlist document operations and CFG sync service. Never modify or
  track this directory.

## Restored workflows

- Spots belong directly to maps. No playlist selection, forms or management.
- Manual spot creation, read/edit and delete. Add works on an empty map.
- Click selects locally without a timer or request. Double-click/Enter opens
  details with Save/Delete; no hover previews or order controls. Hovering a spot
  row or individual map marker shows its full name in a tooltip.
- Opening a spot list selects its first item by default; an explicit valid spot
  selection is preserved during polling and navigation.
- The browser context menu is disabled. Right-click a spot in the list or on the
  map for Copy setpos, which copies its complete setpos/setang command. The menu
  closes on outside click or Escape; polling leaves it intact while open.
- Saved and Captures show all placed spots for the current map directly in the
  left list. There are no zone fills or zone filters; legacy area parameters are
  ignored. Saved uses circular count markers to group nearby spots on the map.
- A detected supported CS2 map gives the map selector a green outline.
- Mode: Captures switches the map and list to only this map's pending buffer,
  showing spots directly without grouping, using cyan/green points. Saved markers
  use blue with amber selection. Save accepts; Delete discards.
  The mode menu lists Saved before Captures. Add spot is disabled in Captures;
  manual creation is available in Saved, while captures come from CS2.
- Ctrl+S explicitly exports the current map's saved spots and current selection.
  Pending captures must be accepted first. No Sync or Save + sync buttons/actions.
- Add spot and Mode sit together at top-center, in that order. Theme stays
  top-right. The logo and Utilizer name sit bottom-left, followed by a question-mark icon
  opening Binds. The modal contains only Capture spot (F6 by default): click its key,
  press a single supported keyboard key, Mouse 4/5, or scroll up/down over the field,
  then Save. Escape cancels recording;
  Tab moves focus. Modifiers and the reserved arrow keys are rejected. The setting
  is global and uses the existing settings table with stale-edit protection.
  Save changes only the setting; Sync in Saved mode writes the new capture key.
  Prior owned capture keys are cleared, including across multiple unapplied Syncs.
  Dialogs close on backdrop click or Escape, without a cross.
- Bottom-center reusable chips last three seconds with a duration line; important
  errors remain until interaction outside. Chips have no close button. Reduced motion disables the animation.
- Saving/deleting never exports. Explicit sync protects unrelated cfg files.
- The Test expander, sample controls and `/__dev/capture` endpoint are removed.
  Tests inject only temporary log fixtures; preview remains isolated.

## Validation and state

Spot names are 1-160 characters, without control characters.
Position commands are at most 256 bytes, parsed as exactly setpos/setang numeric
triples. Positions must fit the map radar and supported numeric ranges. Reference
URLs are bounded to 2048 bytes and http/https only. Templates escape user data;
list names ellipsize and fields cannot expand the surrounding layout.
Origin/Host checks, production authentication, request size limits and CSP remain.

Dialog forms sit outside the polled map and are protected from refreshes; duplicate
in-flight submissions are blocked. Revision stamps reject stale updates from
another window. The first-version database is initialized from `src/schema.sql`,
with no migrations or prototype data conversion. Active storage uses `map_spots`
and non-reused IDs. Capture acceptance is transactional. Earlier
prototype databases are unused; the application starts with `spots.sqlite3`.
Legacy playlist library helpers remain independently usable by core regression
checks, but the application neither creates nor uses stored playlists.

## Compiled CS2 state

Active Sync writes exactly one `cfg.cfg` at the cfg root, with ordered `ut_N`
aliases, capture, selection and all four arrow bindings. `exec cfg` loads without
teleporting; `ut_7` jumps via an alias. No numbered cfg files are generated.
Left/right wrap; Up reloads `cfg.cfg` then jumps to the latest synced selection;
Down executes `setang 89 <current spot yaw> 0`.
Teleports use Ruby's `say_team [number/total] description`; the capture bind preserves its
capture message and marker/getpos protocol. Run `exec cfg` once to install the
bindings (also once when upgrading from the old repeat-only Up binding).
Subsequent Syncs are picked up with Up, without another console command.
Text escaping, alias bounds, stale-alias clearing, owned-file cleanup and
protection of unrelated files/reparse points remain. Real CS2 behavior still
needs a game check; fixtures verify generation and filesystem boundaries.

## Painting and layout

Saved spots group by screen proximity: a 48px search radius with a fixed seed and
an 80px extent limit prevents chains spanning the map. Membership is deterministic
and recalculated on zoom/resize, while panning and selection keep groups stable.
Overlapping badges are reconciled with a separate 128px combined extent limit,
allowing dense edges to merge without creating unbounded chains.
Circles show the actual spot count, using opaque soft blue and dark navy numbers.
Their diameter grows logarithmically with count, with a small contribution from
spatial spread, capped at 48px. Plex semibold tabular figures stay centered and
scale to fit. A group containing the selected spot is amber with a black outline.
Clicking a group selects its first listed spot and zooms in around the group.
At maximum zoom all groups unfold automatically
into individual markers. Overlapping 22px hit targets spread to at least 24px
separation, with thin leaders back to their real coordinates; saved coordinates
are never modified. Zooming out restores normal grouping and positions.
Keyboard focus survives regrouping and polling.
Groups summarize positions, including across walls or floors, not grenade coverage.

Isolated Saved spots and all Captures use 10px circular markers with a black border
and 22px hit targets, kept constant in screen size during zoom and resize. Selection
changes their fill to amber for Saved or green for Captures. Every spot remains
available in the left list at every zoom level.

Artwork is not modified. The viewBox trims transparent padding using the pinned
PNG artwork bounds plus 8 units; original radar coordinates and calibration stay
unchanged. The workspace fills the window width and height. The independently scrolling
list has a bounded width; the SVG fits the remaining space in both dimensions
while preserving its aspect ratio. Long names truncate inside their own column.
Scroll or the +/− controls zoom from the fitted map up to 8×; drag pans within
the map bounds. Fit restores the full map. With the map focused, +/− zoom,
arrow keys pan and 0 fits. Navigation stays local per map across polling,
mode changes and theme changes for the lifetime of the page. A drag does
not select a spot, and polling cannot replace the map during a drag.

## Remaining reference differences and limitations

The original Ruby file-based CSV workflow and map/batch sync are not claimed as
Rust parity. Nuke still has only upper artwork. Map/calibration alignment needs
real-game checks. Current log entry detection uses confirmed Client Map lines,
clears state on loading/disconnect and starts at EOF; synthetic getpos fixtures
still do not prove the actual CS2 capture output format. Only explicit actions
export cfgs; startup never changes game files. No new release is authorized.
