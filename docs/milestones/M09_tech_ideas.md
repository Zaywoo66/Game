# M9 Technology and ideas
Docs: DATA_FORMATS.md, SIM_RULES.md

Progress: technology YAML v1 contract, prerequisite-DAG data linter, typed
Rust loader, research-point/unlock slice, and data-positioned technology UI
are implemented. National ideas have tag-keyed YAML catalogs, linter coverage,
one-time purchases, permanent modifier sources, save v13/LAN v11 transport,
profile-weighted AI selection, and a host-command UI panel. Government reforms
now have a data catalog, one-per-tier purchases, permanent modifier sources,
save v14/LAN v12 transport, profile-weighted AI selection, and a host-command
UI panel. Needs-aware tech/idea weights now use explicit war and economy rules.
The research slice grants 10 points at each new calendar month, caps unspent points at
1000, and spends the stated cost through an authorized `UnlockTechnology`
command. Technology modifiers are permanent and retain their source IDs.
The national-idea slice adds optional country-tagged YAML catalogs, one-time 100-point
purchases from the same stockpile, permanent modifiers and profile-weighted
AI choices through a validated command.
During war, military-branch technologies and `army_morale` ideas receive double
their profile weight. When forecast monthly net balance is negative or the
treasury is below forecast monthly expenses, administration technologies and
`tax_income` ideas receive double weight. Other weights are unchanged; reforms
remain profile-only. Tests cover both need conditions and matching/nonmatching
weights. The England and France files exercise the legacy mechanic. Technology
branches are shared by every country and now use modern administration,
economy, and military content. Countries without a unique catalog use
`data/ideas/common.yaml`; a country-tagged file replaces that fallback. The
2022 unique set is France, Britain, Germany, Italy, Spain, Portugal, the
Netherlands, Sweden, Denmark, Austria, Poland, Hungary, Greece, Turkey, Russia,
and Saudi Arabia. Greece's catalog is Byzantine-themed and the Saudi catalog
retains the Arab Caliphate idea. Legacy English and French catalogs remain for
the 1440 scenario. Current ideas grant permanent modifiers; forming a new
country tag or implementing the distinct institutions in the design table is
not supported by the current idea-effect format. M9 acceptance and the AI-only
progress run remain open.

## Tasks
- Tech DAG loader, research points, unlock effects as modifiers
- National-idea and government-reform systems; defer per-country idea/modifier tuning until after alpha
- Tech tree window drawn from node positions in data

## Acceptance
- Data linter detects cycles and missing prerequisites; modifiers show in tooltips with sources

## AI slice (required)
Docs: AI.md
- **AI L3 (tech/ideas)**: choose technologies and ideas according to profile and current needs (war, economy)
- Acceptance add-on: AI-only run shows tech progress for all countries within sane bounds
