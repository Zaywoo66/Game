# M11 Beta content (1440)
Docs: DATA_FORMATS.md, MAP_PIPELINE.md, ASSETS.md

## Tasks
- Expand map to beta region (~300-500 provinces); 1440 borders from 1400 snapshot + references
- Countries, rulers, dynasties, starting diplomacy; unique ideas for major countries
- Events and tech coverage up to ~1500; flags via heraldry generator

## Acceptance
- Data linter green; every country has capital, ruler, color, flag
- Tracked per-batch (countries by region) so work can be split across small PRs

## AI slice (required)
Docs: AI.md
- Each country needs an AI profile key (archetype) and optional overrides in its data file
- Linter fails if a country lacks an AI profile
- Tune profiles so historical big powers behave plausibly (soak statistics logged)
