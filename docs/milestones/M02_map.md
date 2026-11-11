# M2 Map
Docs: MAP_PIPELINE.md, DATA_FORMATS.md

## Tasks
- tools/map: generate test region (30-50 provinces) -> provinces.png/json, heightmap, terrain
- Load map in /ui/map: chunked terrain, camera (pan/zoom/tilt)
- Province picking via raster; owner coloring + borders via lookup texture
- Water shader; sea provinces visible

## Acceptance
- 60 FPS at 1080p on integrated GPU; RAM measured and logged
- Clicking any province returns correct id (test with script)
- Changing owner updates colors without texture regeneration
