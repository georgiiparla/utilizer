# Frontend rebuild checklist

The current interface keeps the map, zone-filtered left list, selectors, Add spot and
a bottom-left Saved/Captures mode dropdown. Ctrl+S is the only sync action. Light/Dark preferences and matching local map artwork are preserved. Only
Dust2, Mirage, Inferno, Nuke, Ancient, Cache and Anubis are selectable.

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
  details with Save/Delete; no hover previews, titles, copy or order controls.
- Mode: Captures switches the map and list to only this map's pending buffer,
  using cyan/green zones and points. Save accepts; Delete discards.
- Ctrl+S explicitly exports the current map's saved spots and current selection.
  Pending captures must be accepted first. No Sync or Save + sync buttons/actions.
- Add spot and Help sit together at top-center; Theme stays top-right. The mode dropdown
  sits bottom-left. Dialogs close on backdrop click or Escape, without a cross.
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
Left/right wrap; Up repeats; Down executes `setang 89 <current spot yaw> 0`.
Teleports use Ruby's `say_team [number/total] description`; F6 preserves its
capture message and marker/getpos protocol. Reload `exec cfg` after Sync.
Text escaping, alias bounds, stale-alias clearing, owned-file cleanup and
protection of unrelated files/reparse points remain. Real CS2 behavior still
needs a game check; fixtures verify generation and filesystem boundaries.

## Painting and layout

Every projected start contributes an 48-by-48 square. Touching/overlapping squares
within 64 Z units join a group. Paint uses one path per group with unchanged
nonzero fill, clipped by the corresponding local SVG base outline and even-odd
cutouts. Dark and light geometry match. Zones are quick-access groups, including
across walls; they do not imply walkability or grenade coverage.

Artwork is not modified. The viewBox trims transparent padding using the pinned
PNG artwork bounds plus 8 units; original radar coordinates and calibration stay
unchanged. The workspace fills the window width and height. The independently scrolling
list has a bounded width; the SVG fits the remaining space in both dimensions
while preserving its aspect ratio. Long names truncate inside their own column.

## Remaining reference differences and limitations

The original Ruby file-based CSV workflow and map/batch sync are not claimed as
Rust parity. Nuke still has only upper artwork. Map/calibration alignment needs
real-game checks. Current log entry detection uses confirmed Client Map lines,
clears state on loading/disconnect and starts at EOF; synthetic getpos fixtures
still do not prove the actual CS2 capture output format. Only explicit actions
export cfgs; startup never changes game files. No new release is authorized.
