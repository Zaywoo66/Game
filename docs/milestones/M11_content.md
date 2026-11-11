# M11 Beta content (1440)
Docs: DATA_FORMATS.md, MAP_PIPELINE.md, ASSETS.md

## Tasks
- Expand map to beta region at the density defined in MAP_SCALE.md (~1,200–1,800 provinces). Use the 1400 snapshot and references for 1440 borders.
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

## Content batches
- Brittany, 1440: five Cliopatria v0.1.3 [CC BY 4.0](https://github.com/Seshat-Global-History-Databank/cliopatria/releases/tag/v0.1.3) Duchy of Brittany polygon matches (`brest`, `lorient`, `nantes`, `rennes`, `st_brieuc`) are assigned to the new `bre` country. The generated `polities_1440.json` records overlap evidence; it is not used as an automatic owner table. [Brittany's official heritage inventory](https://patrimoine.bzh/gertrude-diffusion/dossier/IA29001760) documents Duke Jean V's 1399–1442 reign; an [edited historical chronicle](https://bibliotheque.idbe.bzh/data/cle_11_fevrier_2016/cle_14_18_fevrier_2016/Chronique_de_Bretagne_de_Jean_de_Saint_Paul_.pdf) gives his birth date as 24 December 1389. The [Château des ducs de Bretagne](https://www.chateaunantes.fr/wp-content/uploads/2020/06/dp_austria.pdf) identifies Nantes as the dukes' capital. His 3/3/3 skills and starting economy are gameplay tuning values, not historical measurements.
- Open name correction: `lorient` is a stable map key but [the city was founded in 1666](https://patrimoine.lorient.bzh/histoire/lieux/centre-ville/les-quartiers/le-centre-historique). Correct the 1440 display name in a separate map-identity-aware PR. The cell's geographic ownership is supported by the Brittany polygon; this batch does not claim the city existed in 1440.
- Holy Roman Empire minor states, Kalmar Union cells, and unmatched Irish cells remain unassigned pending finer evidence. The four unowned cells broadly matched to the Crown of Aragon also remain unassigned because the sidecar's best-overlap label is not reliable evidence of their 1440 jurisdiction.
