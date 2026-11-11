# MAP_PIPELINE

## Source data (public domain / open licenses; log each in ASSETS.md)
- Natural Earth 5.1.2 provides land, lakes and rivers. NOAA ETOPO1 provides elevation and bathymetry.
- M2 uses the 1440 layer from Seshat Cliopatria 0.1.3 (CC BY 4.0). This replaces the previously proposed historical-basemaps input because Cliopatria has explicit data licensing within the asset policy.

## Tools
- Python (`rasterio`, `shapely`, `scipy`, `numpy`, `Pillow`) in `/tools/map`. QGIS for manual inspection/edits.
- Install the pinned dependencies with `python -m pip install -r tools/map/requirements.txt`.
- Generate the test map offline from the checked-in source subsets with `python tools/map/build.py --region western_europe`. `tools/map/fetch_inputs.py` reproduces the clipped source inputs. Edit inputs or overrides to change generated PNGs.
- Validate the map with `python tools/map/validate.py --region western_europe` and validate its schema and hash with the Rust `lint-provinces` CLI. CI also runs the browser picking and owner lookup tests.

## Outputs (`/data/map/<region>/`)
| File | Meaning |
|------|---------|
| provinces.png | RGB24 ID, `(red << 16) | (green << 8) | blue`, with nearest sampling and no antialiasing |
| provinces.json | see DATA_FORMATS.md; neighbors computed by script |
| heightmap.png | 16-bit grayscale elevation in metres, decoded as sample minus 32768 |
| terrain.png | 8-bit terrain index: 0 sea, 1 plains, 2 hills, 3 mountains, 4 lake |
| rivers.png | 8-bit mask: 0 background, 255 river |
| water mask | derived from sea provinces |

M4 writes symmetric movement costs in the map metadata using the region's
`movement_costs` in `overrides.json`. This changes the metadata hash and map
version while preserving the four generated rasters.

## Province generation
For M5b, follow the [city-seeded Voronoi generation rules](MAP_SCALE.md). Allocate province IDs from reserved ranges for each region and use one global equirectangular coordinate system with fixed pixels per degree and a crop origin for each region.

## Rendering (`/ui/src/map`, three.js WebGL2)
- Terrain mesh from heightmap (chunked), custom shaders: terrain blend, water, province borders, owner color via lookup texture (`provinceId -> ownerColor`).
- Picking: CPU copy of provinces raster, read pixel under cursor -> province id.
- Units: `InstancedMesh`, < 500 triangles each, shared texture palette. Sea units same approach.
- Camera: pan, zoom, tilt limits. Map wraps are NOT supported in beta.
- Update ownership by writing bytes to the lookup texture, never regenerate the map texture.

## Budgets
Province raster for beta region <= ~4096x2048. To measure the 1080p viewport, run the built Windows executable with `--map-benchmark`. It opens fullscreen and displays the actual drawing-buffer size and frame timings. Record measured RAM and FPS in DECISIONS.md with the GPU and method.
