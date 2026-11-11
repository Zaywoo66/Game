# M5b Map density and visual/UI pass (insert between M5 and M6)
Docs: MAP_SCALE.md, MAP_PIPELINE.md; create docs/UI_STYLE.md in this milestone.
Why: M2-M5 acceptance was technical only. Later systems (diplomacy, naval, AI) are tuned against map density, so density changes now, not later. M6 does not start until M5b is accepted.

## A. Province density (tools + data)
- Rebuild the Western Europe + British Isles slice with city-seeded provinces per MAP_SCALE.md (target ~400-600), plus areas.yaml and regions.yaml.
- Reassign start ownership by polygon overlap + overrides; keep existing countries and rulers.
- Calibrate movement speed/edge costs so adjacent-province marches take ~3-10 days; record in DECISIONS.md.
- Re-measure tick time, FPS, RAM at the new density; record.
- Old saves are rejected (map hash); new saves round-trip.

## B. Map visuals
- Camera: map fills the viewport; clamp pan/zoom/tilt; no visible void or skewed edges; map center not hidden by panels.
- Water: replace the repeating stripe pattern with depth-based color and subtle animation; no visible tiling at default zoom.
- Borders: thin province borders, thicker country borders, area borders by zoom; no jagged artifacts at default zoom.
- Terrain: hillshade from heightmap, distinct terrain colors, visible rivers.
- Labels: country names at low zoom, city/province names at high zoom, capital markers.
- Map modes: political and terrain.
- Units: readable army/fleet icons with strength number, selection highlight, path preview.

## C. UI
- Write docs/UI_STYLE.md: color/spacing/typography tokens (Tailwind config), layout grid, panel rules, icon rules.
- Layout: top bar (date, pause/speed, treasury/income, ruler), collapsible side panels, province panel on selection, optional minimap, toasts for events. Panels overlay the map edges without covering the focus area.
- Show localized names everywhere (no raw keys such as `london`, `lancaster`). Data linter fails if a displayed key lacks a localization entry.
- Tooltips on numbers show modifier breakdowns.
- Works at 1280x720 and 1920x1080.

## Acceptance (evidence required)
- Add a dev command capturing screenshots at named camera presets (whole region, Britain close-up, Iberian coast, single province close-up) at 1080p. PR must attach them; the owner reviews. No screenshots = not accepted.
- Owner checklist: no void/skewed plane, no visible water tiling, readable borders, no raw keys in UI, panels do not hide the map center.
- 60 FPS at 1080p on target hardware at the new density; RAM logged.
- Existing tests and AI smoke test still pass.

## Suggested task split (one PR each)
5b.1 docs patch; 5b.2 map rebuild + ownership; 5b.3 movement calibration + measurements; 5b.4 camera/water/borders/terrain; 5b.5 labels, map modes, unit markers; 5b.6 UI layout, UI_STYLE.md, localization of displayed keys.
