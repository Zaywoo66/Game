# MAP_PIPELINE

## Source data (public domain / open licenses; log each in ASSETS.md)
- Natural Earth 5.1.2 provides 1:50m land, lakes and rivers, plus 1:10m populated places for city seeds. NOAA ETOPO1 provides elevation and bathymetry.
- M2 uses the 1440 layer from Seshat Cliopatria 0.1.3 (CC BY 4.0). This replaces the previously proposed historical-basemaps input because Cliopatria has explicit data licensing within the asset policy.

## Tools
- Python (`rasterio`, `shapely`, `scipy`, `numpy`, `Pillow`) in `/tools/map`. QGIS for manual inspection/edits.
- Install the pinned dependencies with `python -m pip install -r tools/map/requirements.txt`.
- Generate the city-seeded slice offline from the checked-in source subsets with `python tools/map/build.py --region western_europe`. `tools/map/fetch_inputs.py` reproduces the clipped source inputs. Run the same build with `--update-scenario` to refresh 1440 ownership and forts.
- Validate the map with `python tools/map/validate.py --region western_europe` and validate its schema and hash with the Rust `lint-provinces` CLI. CI also runs the browser picking and owner lookup tests.

## Outputs (`/data/map/<region>/`)
| File | Meaning |
|------|---------|
| provinces.png | RGB24 ID, `(red << 16) | (green << 8) | blue`, with nearest sampling and no antialiasing |
| provinces.json | Schema v3 graph, projection and area keys. See DATA_FORMATS.md. |
| areas.yaml, regions.yaml | Area and region tables with historical provenance where available. |
| names.en.json | English fallback names for provinces, areas and regions. |
| heightmap.png | 16-bit grayscale elevation in metres, decoded as sample minus 32768 |
| terrain.png | 8-bit terrain index: 0 sea, 1 plains, 2 hills, 3 mountains, 4 lake |
| rivers.png | 8-bit mask: 0 background, 255 river |
| water mask | derived from sea provinces |

The generator writes symmetric movement costs in the map metadata using the
region's `movement_costs` in `overrides.yaml`. Changing these costs changes
the metadata hash, so existing saves are rejected even if the schema version
remains 3. The four rasters are unaffected.

## Province generation
The Western Europe crop uses 32 pixels per degree on both axes, with an
800 × 744 raster and global origin offset `[5344, 984]`. Province IDs
use the reserved European range. Settlement seeds and water seeds are
processed independently of starting ownership.

For the 1440 scenario, assign the largest historical polygon overlap,
apply the sourced overrides and explicitly mark land outside the six
playable countries as unowned. Historical area records refer to cited
jurisdictions. Cell footprints approximate membership and do not trace
historical administrative boundaries. Where local evidence is missing,
an unclassified area preserves the gap.

Island settlements remain provinces. Ports retain contact with the sea.
The dense slice adds no over-water land connections. Sea access is
scheduled for M8.

## Rendering (`/ui/src/map`, three.js WebGL2)
- Terrain mesh from heightmap (chunked), custom shaders: terrain blend, hillshade, rivers, depth-colored water, province/area/country borders, owner color via lookup texture (`provinceId -> ownerColor`).
- An orthographic camera clamps zoom, tilt and pan. A low ocean skirt and a short edge fade cover the finite raster edge in wide or tilted views; land outside the regional source extent is not inferred.
- Picking: CPU copy of provinces raster, read pixel under cursor -> province id.
- Units: `InstancedMesh`, < 500 triangles each, shared texture palette. Sea units same approach.
- Camera: pan, zoom, tilt limits. Map wraps are NOT supported in beta.
- Update ownership by writing bytes to the lookup texture, never regenerate the map texture.
- Capture four repeatable 1920 × 1080 browser screenshots with `cd ui && npm run capture:map`; output is `docs/screenshots/m5b4/`. The capture uses Chromium's software WebGL renderer and is visual review evidence, not a hardware FPS result.

## Budgets
Province raster for beta region <= ~4096x2048. To measure the 1080p viewport, run the built Windows executable with `--map-benchmark`. It opens fullscreen and displays the actual drawing-buffer size and frame timings. Record measured RAM and FPS in DECISIONS.md with the GPU and method.
For repeatable RAM sampling, keep the native benchmark window open and sample its private working set from a separate PowerShell terminal with `Get-Process strategy-app | Select-Object ProcessName,Id,@{Name='PrivateMiB';Expression={[math]::Round($_.PrivateMemorySize64 / 1MB, 1)}}`. Record the FPS text after its 2-second warmup and 10-second measurement, plus this memory sample and the renderer string shown in the app. This native measurement is still required for M5b acceptance.
