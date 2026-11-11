# M13 World map (first post-beta milestone; start only after M12)
Docs: MAP_SCALE.md, MAP_PIPELINE.md, DATA_FORMATS.md, AI.md
Prerequisite: the owner decides D13 (horizontal wrap) at the end of M12, before M13 starts.

## Tasks
- Generate provinces for Asia, Africa, the Americas, Oceania with the same pipeline and reserved id ranges; existing ids unchanged.
- Area/region hierarchy for the whole world.
- Implement the owner's D13 decision on horizontal wrap.
- 1440 ownership: major powers detailed, other states as stub countries (`stub: true`), unowned/native provinces handled explicitly.
- Performance pass at ~5,000 provinces and ~300 countries: tick time, A*, AI operation caps, RAM.
- Camera, labels, minimap scaled to the world.

## Acceptance
- Full-world AI-only soak (100 years, 3 seeds): no panic, no invalid state, tick within budget.
- M12 saves rejected cleanly (map hash); new saves load.
- Data linter: every province owned or flagged unowned; every country has capital, ruler, color, AI profile.

## Next (not scheduled, each needs its own milestone file)
Colonization, trade, religion, era content 1500-1850.
