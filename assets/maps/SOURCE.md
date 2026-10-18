# Local map assets

SVG artwork by [setpos.gg](https://setpos.gg), CC BY 4.0.
Pinned upstream revision: `3abd9974d480a4c62b22e18ec7b262f2ed9630e4`.
The seven served map pairs are unchanged upstream light and dark variants. Calibration comes
from maps.json at the same revision. The complete source snapshot including both variants,
README, calibration and license is in `vendor/cs2-minimaps/`; PNG previews included.
No runtime network fetch is needed. Nuke artwork is upper only. Train, Overpass
and Vertigo are excluded from the selectable catalog because this source has
no artwork for them.

Font: IBM Plex Sans, locally bundled from google/fonts (ofl/ibmplexsans),
SIL Open Font License 1.1, see `assets/fonts/OFL.txt`.

Map and floor alignment still need checks against current-game positions.

The `.outline` files contain the unchanged path data from each SVG's `paint-1`
clip path, which clips its base map artwork. They use the same 1024-square
coordinates and even-odd cutouts; Dust2 and Inferno also preserve their source
path translations through the map metadata. The application uses these local derivatives
to clip painted grouping zones, including their pointer hit regions. They do not
change proximity grouping or provide walkability information. Same CC BY 4.0
attribution and pinned source revision as the SVGs above.

For layout only, each map viewBox uses the alpha bounds of its pinned light
1024 PNG plus an 8-unit margin. The source SVG and calibration are unchanged;
all overlay coordinates stay in the original 1024-square radar space.
