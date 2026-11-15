# MAP_SCALE (province hierarchy, density, world readiness)

## Hierarchy
- **Province** = one settlement (city/town) plus surrounding land/coast. Smallest ownable unit and army-movement node. Fields: key, name_key (the settlement's name), terrain, development, optional port/fort, area.
- **Area** = group of ~4-10 provinces (administration, UI, events).
- **Region** = group of ~4-8 areas (AI strategic goals, events). **Superregion/continent** above.
- A country owns provinces; its capital is a province. Files: `data/map/<region>/areas.yaml`, `regions.yaml`.

## Density targets
- Pace rule: moving between two adjacent land provinces at base speed takes ~3-10 game days. Density and movement speed are calibrated together (M5b).
- Rough counts: England 25-35, France 40-60, Portugal 8-12, city-state 1-2.
- The owner-approved 1440 Europe campaign uses about 3,000 land provinces. Fine subdivisions target 350 latitude-adjusted raster-area units, measured against the mean British Isles cell area. They apply only to the major-country tags in `tools/map/inputs/europe/major_density_1440.json`.
- Countries outside that roster keep their existing province geometry. Do not subdivide microstates or one-cell city-state approximations such as San Marino, Frankfurt, or Ansbach. HRE source cells still receive an owner through the separate HRE member fallback policy. Northern Russian land north of 68°N and east of 30°E is an owner-approved unowned frontier for later colonization; Norway and Sweden are excluded by their country tags. In Game political mode, unowned frontier cells retain physical atlas terrain and relief without a political fill.
- Western Europe + British Isles slice: ~400-600. World: ~4,000-5,000.
- Sea zones are coarser than land (about one third of land density).

## Generation rules (tools/map)
- Seeds = real settlements from Natural Earth populated places (public domain), filtered by importance and region. Add Poisson-disk seeds only where there are no settlements (deserts, mountains, sea).
- Cells = Voronoi from seeds, clipped by coastline, split along major rivers/ridges where possible. Merge cells below a minimum area; split above a maximum.
- Province name = seed settlement name (English), as a localization key. Historical names come later via loc files.
- Start ownership: assign from historical border polygons by maximum overlap, then fix by `overrides.yaml`, then lint (every land province owned or explicitly unowned; capitals exist and belong to their country).
- Pipeline must work for N up to ~6,000 provinces without code changes.

## World readiness (do now, even though only one region is generated)
- One global coordinate system: equirectangular (linear lon/lat), fixed pixels-per-degree. Every region is a crop with an origin offset, so regions merge without regeneration. The current combined campaign canvas spans lon −18…68 and lat 10…72 at 32 pixels/degree.
- Province ids allocated from reserved ranges (existing Europe IDs 1–3999; appended North Africa and Middle East IDs 4000–5999; later regions start at 6000). Adding a region never renumbers existing IDs. Map version/hash is stored in saves; saves from other map versions are rejected.
- Terrain is chunked (e.g. 512x512 tiles); no code assumes the whole heightmap is one mesh.
- Memory targets (confirm by measurement): beta region map rasters <= 64 MB resident; world <= 150 MB.
- The owner decides whether the world map wraps horizontally at the end of M12, before M13 begins (DECISIONS D13). M13 implements that decision.
- Before M13 is accepted: benchmark tick, A*, and AI operation caps at ~5,000 provinces and ~300 countries.

## Content tiers for far regions
Major powers: fully detailed. Minor/tribal states: generated stub countries (`stub: true`: name, color, ruler, AI profile), refined in later content batches.
