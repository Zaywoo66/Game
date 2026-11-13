# M5b Map density and visual/UI pass
Docs: MAP_SCALE.md, MAP_PIPELINE.md; create docs/UI_STYLE.md in this milestone.
Why: M2-M5 acceptance was technical only. Later systems (diplomacy, naval, AI) are tuned against map density, so the 5b.2 rebuild is verified before M6. The remaining M5b visual/UI work may continue alongside M6–M11. M12 waits for M5b acceptance.

## A. Province density (tools + data)
- Rebuild the Western Europe + British Isles slice with city-seeded provinces per MAP_SCALE.md (target ~400-600), plus areas.yaml and regions.yaml.
- Reassign start ownership by polygon overlap + overrides; keep existing countries and rulers.
- Calibrate movement speed/edge costs so adjacent-province marches take ~3-10 days; record in DECISIONS.md.
- Re-measure tick time, FPS, RAM, full LAN snapshot size and delta traffic at the new density. Record snapshot bytes and delta bytes over a stated interval in 5b.3.
- Old saves are rejected (map hash); new saves round-trip.

## B. Map visuals
- Camera: map fills the viewport; clamp pan/zoom/tilt; no visible void or skewed edges; map center not hidden by panels.
- Water: replace the repeating stripe pattern with depth-based color and subtle animation; no visible tiling at default zoom.
- Borders: thin province borders, thicker country borders, area borders by zoom; no jagged artifacts at default zoom.
- Hover: lift the hovered province and show its localized name after a short delay; selection uses a distinct outline and subtle pulse.
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

### Interaction progress
- Camera keyboard pan (WASD/arrows), Home framing, double-click province focus,
  and cursor-centered wheel zoom are implemented. Remaining 5b.7 controls,
  order feedback, notifications, settings, and session acceptance are open.
- The map UI now constrains its header and inspector on laptop-sized windows and
  reduces army/fleet badges to compact icons at overview zoom while retaining
  full strength text for screen readers and tooltips. A 1100×768 browser check
  covers these responsive rules; owner review and native target-hardware review
  remain required.

## Acceptance (evidence required)
- Add a dev command capturing screenshots at named camera presets (whole region, Britain close-up, Iberian coast, single province close-up) at 1080p. PR must attach them; the owner reviews. No screenshots = not accepted.
- Owner checklist: no void/skewed plane, no visible water tiling, readable borders, no raw keys in UI, panels do not hide the map center.
- 60 FPS at 1080p on target hardware at the new density; RAM logged once after
  the final map density/rendering pass and before release. Repeat only if map
  density or rendering changes; unrelated mechanics edits do not trigger it.
- Existing tests and AI smoke test still pass.

## Suggested task split (one PR each)
5b.1 docs patch; 5b.2 map rebuild + ownership; 5b.3 movement calibration + tick, snapshot and delta measurements; 5b.4 camera/water/borders/terrain + native 1080p FPS/RAM after the visual changes; 5b.5 labels, map modes, unit markers, hover/selection feedback; 5b.6 UI layout, UI_STYLE.md, localization of displayed keys; 5b.7 interaction layer; 5b.8 war/peace and playable-session acceptance.
