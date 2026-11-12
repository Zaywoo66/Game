# M9 Technology and ideas
Docs: DATA_FORMATS.md, SIM_RULES.md

Progress: technology YAML v1 contract, prerequisite-DAG data linter, typed
Rust loader, and the research-point/unlock slice are implemented. National
ideas are implemented; reforms, tech-tree UI, and needs-aware AI weights remain open. The research
slice grants 10 points at each new calendar month, caps unspent points at
1000, and spends the stated cost through an authorized `UnlockTechnology`
command. Technology modifiers are permanent and retain their source IDs.
The national-idea slice adds country-tagged YAML catalogs, one-time 100-point
purchases from the same stockpile, permanent modifiers and profile-weighted
AI choices through a validated command.
The England and France files exercise the mechanic; historical idea catalogs
for the remaining 1440 countries belong to M11 content work. M9 remains open.

## Tasks
- Tech DAG loader, research points, unlock effects as modifiers
- Unique ideas per country, government reforms
- Tech tree window drawn from node positions in data

## Acceptance
- Data linter detects cycles and missing prerequisites; modifiers show in tooltips with sources

## AI slice (required)
Docs: AI.md
- **AI L3 (tech/ideas)**: choose technologies and ideas according to profile and current needs (war, economy)
- Acceptance add-on: AI-only run shows tech progress for all countries within sane bounds
