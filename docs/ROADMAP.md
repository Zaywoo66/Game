# ROADMAP (index only — details live in milestones/)
Status: [ ] todo, [~] in progress, [x] done. Update in the PR that changes it. Work milestones in order except independent M6–M11 tasks explicitly authorized to proceed in parallel; M12 still requires M5 and M5b acceptance.

| ID | Milestone | Status | File |
|----|-----------|--------|------|
| M0 | Foundation | [x] | milestones/M00_foundation.md |
| M1 | Simulation core | [x] | milestones/M01_sim_core.md |
| M2 | Map | [x] | milestones/M02_map.md |
| M3 | Countries and rulers | [x] | milestones/M03_countries_rulers.md |
| M4 | Armies and land war | [x] | milestones/M04_armies_war.md |
| M5 | LAN multiplayer | [~] | milestones/M05_multiplayer.md |
| M5b | Map density and visual/UI pass | [~] | milestones/M05b_visual_pass.md |
| M6 | Diplomacy | [~] | milestones/M06_diplomacy.md |
| M7 | Generic events and RP (country-specific event packs post-alpha) | [~] | milestones/M07_events_rp.md |
| M8 | Naval | [~] | milestones/M08_naval.md |
| M9 | Technology and ideas | [~] | milestones/M09_tech_ideas.md |
| M10 | AI strategic layer, balance, tuning (L4) | [~] | milestones/M10_ai.md |
| M11 | Campaign content (2022 production scenario; 1440 retained as legacy) | [~] | milestones/M11_content.md |
| M12 | Beta release | [ ] | milestones/M12_release.md |
| M13 | World map (post-beta; trade slice brought forward) | [~] | milestones/M13_world.md |

## Alpha readiness ledger

Update this ledger with implementation changes; keep milestone status `[~]`
until the listed acceptance evidence is complete. Do not treat a passing local
build as proof of physical LAN or owner visual acceptance.

| Work slice | Implementation status | Still open before acceptance |
|---|---|---|
| M0–M4 foundation, simulation, map, countries, land war | Complete `[x]` | None in these milestones. |
| M5 LAN foundation | Protocol, host/client, discovery, reconnect, checksums, resync and LAN log mode implemented. The app opens at a title screen; solo play now selects a country and starts only after the player confirms, with other countries AI-controlled. Network Game offers Create/Join Room; the host chooses a country when creating a room, the guest claims one after joining, and the host starts after both seats are assigned. | Complete the two-PC hour run and record second PC GPU/FPS and discovery result in D4. |
| M5b map and UI | Europe map pipeline, polygon evidence view, ownership coloring, map modes, interaction, unit markers and panels implemented. Recent screenshot corrections enlarge the regional map to use the window height, improve relief and mute political fills, keep recruiting armies legible, and hide raw sea-zone keys. HUD is compact; the right inspector can collapse; development links are hidden from production UI. Native current-map benchmark recorded 144 FPS / p95 7.0 ms at 1920×1080 on RTX 4050; process tree measured 794.0 MiB working set / 662.3 MiB private memory. | Owner review of current screenshots; integrated-GPU/target-hardware confirmation; remaining interaction/localization work. Do not mark accepted until owner visual review. |
| M6 diplomacy | Alliances, marriages, guarantees, vassalage, multi-participant wars, truces, call-to-arms, family growth, named aristocratic claimants, and succession-conflict rules implemented on the current branch. | Run the full checks and verify save/LAN/UI handling; complete two-player proposal/response acceptance and succession-war playthrough. |
| M7 generic events | Deterministic engine, choices, AI weighting, generic 1440 pack and English/Russian catalogs implemented. | AI-only event run and both-player multiplayer resolution acceptance. Historical country packs are post-alpha. |
| M8 naval | Fleets, transport, combat, blockade, attrition, construction and AI landing implemented; focused tests pass. | Complete AI L3 acceptance run and content/starting-port review. |
| M9 research and government | Shared modern administration, economy and military technologies; common idea fallback; country-specific modern idea catalogs; research, purchases and reforms are implemented. | Implement real formable-country and institution mechanics represented by the idea design; finish government-transition UI/data; run AI-only progress acceptance. |
| M10 strategic AI | Reason-coded decisions, coalition-aware war scoring, call-to-arms, needs-aware weights and soak-report harness implemented. Aggressive-expansion counters decay by 1 per peaceful month. | AE gain sources and sanction decisions remain unspecified; long-term goal/difficulty rules, five-seed 100-year soak and balance review remain open. Do not guess missing rules. |
| M11 legacy 1440 content | Historical campaign remains available to tests and content tools: 72 countries and the existing 3,039-province map/content. | No longer the default campaign; retained for compatibility while modern 2022 scenario receives final content and visual review. |
| M11 modern 2022 conversion | Implemented: default local and LAN startup uses a 2022 scenario; 90 countries own all 3,468 land provinces on combined Europe-North Africa-Middle East map v9. Natural Earth Ukraine POV provides Crimea borders; exact-name settlement cells and country-fill/border overview layers are included. North African and Middle Eastern leaders use year-precision birth dates where applicable; shared technologies and baseline mechanics remain common. | Owner review of native map, country labels/colors, scenario data, and leader roster; the new extension currently has flat placeholder elevation; modern province ownership remains a raster approximation and tiny states may be merged. |
| Alpha packaging | Windows MSI and NSIS setup executable configured in Tauri; both local bundles built successfully. CI is configured to upload them as `strategy-windows-installers`. | Push/merge the packaging change and confirm its CI artifact is downloadable. Native launch/GPU smoke and player-facing release notes remain open. Beta remains gated by M5/M5b acceptance in M12. |

**Current campaign rule (owner clarification, 2026-11-14):** the production
campaign starts on 2022-01-01, using modern sovereign borders. Keep core game
mechanics unchanged. The previous 1440 campaign remains a legacy scenario for
tests/content comparison and is not the default. There is one start date; do
not add era selection. Use the Natural Earth 2022 release for the modern
political layer and record approximation/coverage limits. This supersedes the
older 1440 alpha scope below.

**Legacy 1440 content rule:**
Do not add country-specific historical events or tune unique national buffs
and debuffs before alpha. Defer those country content packs to post-alpha
patches. Keep the already implemented generic event and idea systems available.
The existing generic event pack and baseline England/France idea catalogs
remain as mechanics examples; do not expand or retune them for alpha.
Technology branches are shared by every country. In the legacy 1440 campaign,
the existing country-specific idea catalogs remain available and other
countries use the common fallback. In the 2022 campaign, the common fallback
is used unless a unique catalog exists for France, Britain, Germany, Italy,
Spain, Portugal, the Netherlands, Sweden, Denmark, Austria, Poland, Hungary,
Greece, Turkey, Russia, or Saudi Arabia. The Saudi catalog retains the
Arab-Caliphate idea; Greece's is Byzantine-themed. These idea catalogs grant
modifiers only; actual country-tag formation and the listed institutions are
not yet supported.
The legacy campaign data below retain their original 1440 date and source notes.

M5b political-source audit: generated 1440 overlap evidence covers every
land province and marks unknown or coarse matches. The Cliopatria v0.2.1
refresh applies a documented Bavaria-Straubing/Holland correction; map
identities are now Western Europe v4 and Europe v7. Scenario ownership and
visual acceptance remain in progress. Game political view uses country colors
only for owned provinces; unowned northern frontier cells display physical
terrain/relief without political tint. Historical 1440 source colors remain a
separate inspection mode.

2026-11-15 visual pass: political ownership fills now retain restrained relief
shading and elevation tint, while province and area lines are lighter. Map
ownership and source geometry are unchanged; owner visual acceptance remains
open.

AI levels L0–L3 are delivered inside M3, M4, M6, M7–M9 (see each milestone's AI slice); M10 only completes L4.

After the 5b.2 dense-map rebuild passes CI, M6–M11 may proceed while physical two-PC M5 acceptance and the remaining M5b visual/UI checks are open. M12 cannot start until M5 and M5b are accepted. Local loopback and accelerated runs support development but do not close the two-PC acceptance.
The owner authorized independent small M6–M11 PRs in parallel on 2026-11-11.
For the current alpha integration branch, related implementation changes may be
batched in a draft PR. Full CI is a review-readiness gate, not an after-every-edit
loop; run focused checks only to diagnose a concrete issue. Keep long soaks and
hardware/LAN acceptance at their milestone or release gates.
M7 has a typed event loader, deterministic daily firing, pending choices, AI
weighted responses, and host-validated choice commands. Its 20-event generic
starter pack has complete English and Russian catalogs; browser language
selects the displayed catalog. The generic pack is sufficient for alpha;
country-specific historical events and content authoring are deferred to
post-alpha patches. AI-only event-run and multiplayer resolution checks remain
verification work for the later validation pass; no event-writing task blocks
map completion.
The M9 technology YAML contract and typed loader validate branch files, effect
units, and the cross-branch prerequisite DAG. Research adds monthly points,
permanent modifiers, profile-weighted AI choices and a data-positioned tree UI.
National ideas have tag-keyed catalogs, one-time purchases, permanent modifier
sources, save v13/LAN v11 transport, profile-weighted AI choices and a host-
command UI panel. Government reforms now have a data catalog, 150-point
one-per-tier purchases, permanent modifier sources, save v14/LAN v12 transport,
profile-weighted AI choices and a host-command UI panel. Needs-aware AI now
doubles matching military priorities during war and administration/tax
priorities for projected economic need; focused tests cover both conditions.
Historical catalogs for the remaining 1440 countries and the AI-only M9
acceptance run remain open.
The current UI/military follow-up moves country-to-country diplomacy into the
selected country card, adds right-drag army group selection, limits AI offensive
wars to hostile relations after a 365-day opening peace period, and gives new
armies a 30-day recruitment countdown. These changes do not close M10/M12 or
replace multiplayer acceptance.
M5b.4 camera, water, borders and terrain have browser screenshot evidence. The map camera is now locked to a top-down view; left- and right-drag pan, while wheel zoom remains available. On 2026-11-13 the native Tauri app measured 144 FPS (p95 7.0 ms) at 1920×1080 on the RTX 4050; a process-tree sample measured 794.0 MiB working set and 662.3 MiB private memory. The integrated GPU is unavailable on this PC. Owner visual review remains open for M5b acceptance.
M5b.5 map modes, historical evidence display, English labels, army/fleet badges, fleet route preview, province hover highlighting, delayed name tooltips, and selected-province outline/pulse are implemented. The responsive UI pass constrains overlays on laptop windows and changes army/fleet badges to compact icons at overview zoom; browser coverage checks the 1100×768 layout and accessible strength labels. Named polygons use sourced polity colors; coarse/unmatched land receives striped per-province placeholder tints with no country label, so no single sovereign is implied. Russian localization, native performance, screenshot review and owner visual acceptance remain open.

The modern production scenario was added on 2026-11-14. Map v9 extends the Europe graph south to lat 10 and west to lon −18, preserving every prior Europe province ID; new North African and Middle Eastern cells occupy the 4000–5999 range. Natural Earth Admin-0 sovereignty groups (release 5.1.1, 2022) assign every land province; the scenario has 90 countries and 3,468 owned land cells, including a dedicated Beirut province for Lebanon. The campaign starts 2022-01-01. The new extension uses placeholder flat elevation; owners and borders are raster approximations, not sub-province geometry. North African / Middle Eastern heads of state and government are represented in the 2022 roster with year-precision birth years. Native owner review remains pending.

M6 alliance and royal-marriage slices implemented: validated 30-day offers, acceptance/rejection, spouse links and death cleanup, +20 temporary effective opinion, deterministic AI factors, and network/save transport. The guarantee slice adds unilateral indefinite guarantees, automatic defensive joining, broken-promise penalties and AI offers. Automatic post-war truces are visible in the country and military panels. The vassalage slice adds 30-day offers, 10% monthly tribute, automatic defense, independence wars, AI answers and typed LAN/UI commands. The country-list diplomacy panel projects opinions and alliances, sends alliance proposals and responses through host-validated commands, supports eligible ruler/heir marriage proposals, and displays AI response factors for legal proposals. A targeted app projection test and UI smoke test cover the forecast, including the 10% negative-opinion alliance chance. Full two-player UI acceptance and M6 acceptance remain open.

Dynastic life-cycle work is in progress on `feat/dynasty-succession`: family links and dates, deterministic births, player-created monarchs, heir nomination, stability display/action, and full-annexation loss handling are being added. Claimant/aristocratic succession wars and their territorial victory condition remain open; do not mark M6 complete until that slice and its tests are finished.
The royal-marriage slice uses named adult characters and synthetic tests; M11 now includes Elizabeth of Luxembourg as an eligible female ruler. Prior spouse links remain limited to sourced historical content.
The war-participants prerequisite supports additional countries on either side of one war, with side-based armies, sieges, peace and pairwise truces.
M8 sea graph and strict v3 port validation are complete. The fleet/transport slice adds deterministic sea movement, port embarkation and landing, and an AI amphibious landing test. The naval combat slice resolves opposing fleets and transport losses. The blockade slice suppresses tax from unopposed wartime blockaded ports. Monthly sea attrition now removes `ceil(ships / 100)` ships outside an accessible port after movement and battles; fleet and transport-army losses are tested. Original instanced low-poly fleet models and validated 30-day ship construction at owned ports are implemented; full AI L3 acceptance remains open.
The 1440 scenario currently starts with neutral opinions; historically supported positive opening relationships and their values belong to M11 content review. Until then, the alliance AI's favorable branch is covered by a synthetic scenario test.
M11 content includes 72 countries, 56 character records and registered banners for all 72. Full Europe v6 had 1,284 owned land provinces. The refined v7 campaign has 3,039 land provinces: 3,006 owned, and 33 northern Russian frontier cells left unowned for later colonization. The v6 source layer labels 145 base cells `Novgorod Republic`; the separate Novgorod country remains under posadnik Esif Grigorievich, with his 1400 birth year as an explicit gameplay estimate. The D19 frontier rule supersedes blanket ownership of Russian land north of 68°N. The 138 base Kalmar-labelled cells are partitioned among Denmark, Norway, and Sweden. A local comparison with AoC2's 1440 scenario led to 80 high-confidence steppe owner corrections and 27 additional minor states owning 83 cells. Original AoC2 files are not distributed. Named-city exceptions and disputed/mixed cells, including Bremen and Hamburg, were reviewed against public sources and retained as documented gameplay approximations where public evidence did not resolve cell-level borders. M11 content acceptance is complete; M5b visual acceptance remains separate. Country-specific event packs and modifier tuning remain deferred until after alpha. Existing AI smoke measurements refer to earlier country snapshots and are development evidence, not M10 acceptance.

M13 is the first post-beta milestone; colonization, trade, religion and later eras follow.
M10 has a yearly multi-seed headless soak report harness and war target scoring
that accounts for guarantees and overlords which can automatically join the
defense. The profile `naval` weight now scales deterministic naval-planner
windows so the data field affects shipbuilding, transport and blockade choices.
Call-to-arms proposals, AI responses, human response commands and
competing-offer closure are implemented over LAN protocol v15. Other L4
behavior and balance acceptance remain open as opt-in release checks; long
soaks and hardware measurements are not repeated for unrelated edits.
Vertical slice = M2–M7 on the small test region (see DECISIONS D5), then expand to beta map in M11.

Brandenburg adds three majority-overlap source cells, Berlin as capital, and Friedrich I as the ruler on 1 January 1440; his documented death is 20 September 1440. The country is modeled as an independent elector-state, not as a unified Holy Roman Empire. Bosnia adds three majority-overlap cells including the raster cell containing Bobovac, under Tvrtko II (c. 1380; reign 1421–1443). The v6 baseline ownership list reconciled 1,284 land provinces; the v7 map adds density refinements and the explicitly unowned northern frontier.
 The House of Bourbon is also represented with Charles I, and its three majority-overlap provinces; Moulins falls in the Vichy raster cell, which is used as the capital province.
 The Empire of Trebizond adds its capital cell `trabzon` and a second majority-overlap cell. John IV ruled from 1429 to 1460; birth is recorded as circa 1404 at year precision using the Great Russian Encyclopedia account, while the St Andrews study gives a conflicting upper-bound date before 1403.
 Additional supported minor states now include the County of Nevers (Charles I), County of Savoy (Amadeus VIII, still duke on the January 1 start), and Duchy of Luxembourg (Elisabeth of Görlitz), each with its capital inside a majority-matched source cell.
