# Map scale patch — apply to shared docs (PR `docs/map-scale`), then delete this file
Do not change existing statuses/measurements. The owner explicitly authorizes the AGENTS.md edit.

## AGENTS.md
Add row to "Which doc to open": `| province density, hierarchy, world readiness | docs/MAP_SCALE.md |`

## docs/ROADMAP.md
- Insert row after M5: `| M5b | Map density and visual/UI pass | [ ] | milestones/M05b_visual_pass.md |` and the rule "M6 does not start until M5b is accepted".
- Add row after M12: `| M13 | World map (post-beta) | [ ] | milestones/M13_world.md |`.
- Replace the post-beta note with: "M13 is the first post-beta milestone; colonization, trade, religion and later eras follow."

## docs/DECISIONS.md
- D5: update to "slice target ~400-600 provinces (supersedes ~100, which was too coarse)".
- D7: beta region unchanged, density per MAP_SCALE.md; world map is M13 right after beta.
- Add Decided D12: province = settlement + surroundings; hierarchy province -> area -> region (MAP_SCALE.md).
- Add Open D13: horizontal wrap on the world map (ASSUMED: decide in M13).
- Add Open D14: far-region countries as stub countries first (ASSUMED yes).

## docs/MAP_PIPELINE.md
Replace the "Province generation" paragraph with a pointer to the generation rules in MAP_SCALE.md (city-seeded Voronoi). Add: ids allocated in reserved ranges per region; one global equirectangular coordinate system.

## docs/DATA_FORMATS.md
Add: province fields `name_key`, `area`; files `areas.yaml`, `regions.yaml`; country field `stub: bool`.

## docs/milestones/M02_map.md
Add note: "the 30-50 province test region was a technical test; production density is defined in MAP_SCALE.md and delivered in M5b."

## docs/milestones/M11_content.md
Replace "~300-500 provinces" with "density per MAP_SCALE.md (~1,200-1,800 for the beta region)".
