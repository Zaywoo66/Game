# MAP_SCALE (province hierarchy, density, world readiness)

## Hierarchy
- **Province** = one settlement (city/town) plus surrounding land/coast. Smallest ownable unit and army-movement node. Fields: key, name_key (the settlement's name), terrain, development, optional port/fort, area.
- **Area** = group of ~4-10 provinces (administration, UI, events).
- **Region** = group of ~4-8 areas (AI strategic goals, events). **Superregion/continent** above.
- A country owns provinces; its capital is a province. Files: `data/map/<region>/areas.yaml`, `regions.yaml`.

## Density targets (INDICATIVE, confirm by measurement in M5b and log in DECISIONS.md)
- Pace rule: moving between two adjacent land provinces at base speed takes ~3-10 game days. Density and movement speed are calibrated together (M5b).
- Rough counts: England 25-35, France 40-60, Portugal 8-12, city-state 1-2.
- Western Europe + British Isles slice: ~400-600. Beta region (Europe + Mediterranean + Near East): ~1,200-1,800. World: ~4,000-5,000.
- Sea zones are coarser than land (about one third of land density).

## Generation rules (tools/map)
- Seeds = real settlements from Natural Earth populated places (public domain), filtered by importance and region. Add Poisson-disk seeds only where there are no settlements (deserts, mountains, sea).
- Cells = Voronoi from seeds, clipped by coastline, split along major rivers/ridges where possible. Merge cells below a minimum area; split above a maximum.
- Province name = seed settlement name (English), as a localization key. Historical names come later via loc files.
- Start ownership: assign from historical border polygons by maximum overlap, then fix by `overrides.yaml`, then lint (every land province owned or explicitly unowned; capitals exist and belong to their country).
- Pipeline must work for N up to ~6,000 provinces without code changes.

## World readiness (do now, even though only one region is generated)
- One global coordinate system: equirectangular (linear lon/lat), fixed pixels-per-degree. Every region is a crop with an origin offset, so regions merge without regeneration.
- Province ids allocated per region from reserved ranges (e.g. Europe 1-1999, next regions after). Adding a region never renumbers existing ids. Map version/hash is stored in saves (saves from other map versions are rejected).
- Terrain is chunked (e.g. 512x512 tiles); no code assumes the whole heightmap is one mesh.
- Memory targets (confirm by measurement): beta region map rasters <= 64 MB resident; world <= 150 MB.
- Horizontal wrap-around: not supported until M13 decides (DECISIONS D13).
- Before M13 is accepted: benchmark tick, A*, and AI operation caps at ~5,000 provinces and ~300 countries.

## Content tiers for far regions
Major powers: fully detailed. Minor/tribal states: generated stub countries (`stub: true`: name, color, ruler, AI profile), refined in later content batches.
