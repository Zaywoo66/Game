# MAP_PIPELINE

## Source data (public domain / open licenses; log each in ASSETS.md)
- Natural Earth: coastline, rivers, borders. GEBCO or ETOPO: elevation and bathymetry.
- `historical-basemaps` (GitHub): starting point for historical borders; 1440 borders are hand-adjusted from the 1400 snapshot.

## Tools
- Python (`rasterio`, `shapely`, `scipy`, `numpy`, `Pillow`) in `/tools/map`. QGIS for manual inspection/edits.
- All generation is scripted and reproducible: `python tools/map/build.py --region <name>`. Never hand-edit generated PNGs; edit the inputs/overrides instead.

## Outputs (`/data/map/<region>/`)
| File | Meaning |
|------|---------|
| provinces.png | RGB color = unique province id (no antialiasing, nearest sampling) |
| provinces.json | see DATA_FORMATS.md; neighbors computed by script |
| heightmap.png | 16-bit grayscale elevation |
| terrain.png | terrain type index per pixel |
| rivers.png | river mask / width |
| water mask | derived from sea provinces |

## Province generation
Voronoi seeded points constrained by coastline and rivers -> merge/split manually via an overrides file (`overrides.yaml`) -> validate (no orphan pixels, no 1-px provinces, all neighbors symmetric, every land province reachable).

## Rendering (`/ui/map`, three.js WebGL2)
- Terrain mesh from heightmap (chunked), custom shaders: terrain blend, water, province borders, owner color via lookup texture (`provinceId -> ownerColor`).
- Picking: CPU copy of provinces raster, read pixel under cursor -> province id.
- Units: `InstancedMesh`, < 500 triangles each, shared texture palette. Sea units same approach.
- Camera: pan, zoom, tilt limits. Map wraps are NOT supported in beta.
- Update ownership by writing bytes to the lookup texture, never regenerate the map texture.

## Budgets
Province raster for beta region <= ~4096x2048. Record measured RAM/FPS in DECISIONS.md at M2.
