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
- Brittany's `bre.svg` is an original ermine design referencing [BnF's 1417–1422 Jean V coin](https://catalogue.bnf.fr/ark:/12148/cb44989244k) and [ducal seal](https://medaillesetantiques.bnf.fr/ws/catalogue/app/collection/record/ark:/12148/c33gb1h34f). Its license and derivation are in `ASSETS.md`; the UI displays it by the `bre` country tag.
- Milan, 1440: `milan`, `como`, and `lugano` have full-cell Cliopatria v0.1.3 polygon matches; `bellinzona` has 637/639 overlap pixels (99.7%). These four cells are assigned to the Duchy of Milan; partial-overlap cells (`parma`, `novara`, `asti`) remain explicitly unowned. [Treccani's biography](https://www.treccani.it/enciclopedia/filippo-maria-visconti-duca-di-milano_%2528Dizionario-Biografico%2529/) identifies Filippo Maria Visconti as born in Milan on 3 September 1392 and Duke from 1412; the [Chazen Museum](https://chazen.wisc.edu/collection/10908/filippo-maria-visconti-duke-of-milan/) catalogs Pisanello's 1441 medal of him. The [Lombardia cultural heritage record](https://www.lombardiabeniculturali.it/opere-arte/schede/3o280-00608/) dates the Visconti arms in the Duchy of Milan to 1395; the game's flag is an original simplified SVG. Country color, ruler skills and economy are gameplay values, not historical measurements.
- Grand Duchy of Lithuania, 1440: 33 full-cell Cliopatria v0.1.3 matches are assigned to `lit`, with Vilnius as capital. Seven generated `eu_*` keys remain mechanically named map cells; the source overlap identifies the polity but does not provide settlement names for those cells. The [National Museum – Palace of the Grand Dukes of Lithuania](https://www.valdovurumai.lt/en/palace-history/i/5363/important-dates/) records Vilnius as capital and Casimir Jagiellon becoming Grand Duke in 1440. A [Jagiellonian University study of his birth horoscope](https://ruj.uj.edu.pl/server/api/core/bitstreams/cc95444d-d338-45d1-99eb-8d3a0aded0af/content) uses 29 November 1427; some modern summaries give 30 November, which does not change his age (12) on 1 January 1440. His skills and starting economy are gameplay values. The flag is an original simplified rider design based on the [Seimas account of the historic Lithuanian coat of arms](https://www.lrs.lt/sip/portal.show?p_k=2&p_kade_id=9&p_r=38110); the source describes the emblem's use from the late 14th century and its colors as established by the early 15th century.
- Open name correction: `lorient` is a stable map key but [the city was founded in 1666](https://patrimoine.lorient.bzh/histoire/lieux/centre-ville/les-quartiers/le-centre-historique). Correct the 1440 display name in a separate map-identity-aware PR. The cell's geographic ownership is supported by the Brittany polygon; this batch does not claim the city existed in 1440.
- Holy Roman Empire minor states, Kalmar Union cells, and unmatched Irish cells remain unassigned pending finer evidence. The four unowned cells broadly matched to the Crown of Aragon also remain unassigned because the sidecar's best-overlap label is not reliable evidence of their 1440 jurisdiction.
