# M2 Map
Docs: MAP_PIPELINE.md, DATA_FORMATS.md

## Tasks
- tools/map: generate a 30–50 province test subset of Western Europe and the British Isles (D5).
  This test region was a technical test. Production density is defined in MAP_SCALE.md and delivered in M5b.
- Generate `provinces.png`, `provinces.json`, `heightmap.png`, `terrain.png`, and `rivers.png`.
- Load map in /ui/map: chunked terrain, camera (pan/zoom/tilt)
- Province picking via raster; owner coloring + borders via lookup texture
- Water shader; sea provinces visible

## Acceptance
- Measure FPS and RAM at 1080p in Tauri on the available Windows PC (D4). Keep 60 FPS on integrated graphics as a performance target and mark that hardware result unverified until measured.
- Clicking any province returns correct id (test with script)
- Changing owner updates colors without texture regeneration
