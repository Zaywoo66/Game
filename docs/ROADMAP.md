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
| M7 | Events and RP | [~] | milestones/M07_events_rp.md |
| M8 | Naval | [~] | milestones/M08_naval.md |
| M9 | Technology and ideas | [~] | milestones/M09_tech_ideas.md |
| M10 | AI strategic layer, balance, tuning (L4) | [~] | milestones/M10_ai.md |
| M11 | Beta content (1440) | [~] | milestones/M11_content.md |
| M12 | Beta release | [ ] | milestones/M12_release.md |
| M13 | World map (post-beta) | [ ] | milestones/M13_world.md |

M5b political-source audit: generated 1440 overlap evidence covers every
land province and marks unknown or coarse matches. Scenario ownership and
visual acceptance remain in progress.

AI levels L0–L3 are delivered inside M3, M4, M6, M7–M9 (see each milestone's AI slice); M10 only completes L4.

After the 5b.2 dense-map rebuild passes CI, M6–M11 may proceed while physical two-PC M5 acceptance and the remaining M5b visual/UI checks are open. M12 cannot start until M5 and M5b are accepted. Local loopback and accelerated runs support development but do not close the two-PC acceptance.
The owner authorized independent small M6–M11 PRs in parallel on 2026-11-11.
M7 has a typed event loader, deterministic daily firing, pending choices, AI
weighted responses, and host-validated choice commands. Its 20-event generic
starter pack has complete English and Russian catalogs; browser language
selects the displayed catalog. Historical event content, an AI-only content
run, and multiplayer acceptance remain open.
The M9 technology YAML contract and typed loader validate branch files, effect
units, and the cross-branch prerequisite DAG. Research adds monthly points,
permanent modifiers, profile-weighted AI choices and a data-positioned tree UI.
National ideas have tag-keyed catalogs, one-time purchases, permanent modifier
sources, save v13/LAN v11 transport, profile-weighted AI choices and a host-
command UI panel. Government reforms now have a data catalog, 150-point
one-per-tier purchases, permanent modifier sources, save v14/LAN v12 transport,
profile-weighted AI choices and a host-command UI panel. Needs-aware AI weights,
historical catalogs for the remaining 1440 countries, and M9 acceptance remain
open.
M5b.4 camera, water, borders and terrain have browser screenshot evidence; native 1080p FPS/RAM and owner visual review remain open for M5b acceptance.
M5b.5 map modes, historical evidence display, English labels, army/fleet badges, fleet route preview, province hover highlighting, delayed name tooltips, and selected-province outline/pulse are implemented. Browser smoke covers province picking and hover tooltip; Russian localization, native performance, screenshot review and owner visual acceptance remain open.

M6 alliance and royal-marriage slices implemented: validated 30-day offers, acceptance/rejection, spouse links and death cleanup, +20 temporary effective opinion, deterministic AI factors, and network/save transport. The guarantee slice adds unilateral indefinite guarantees, automatic defensive joining, broken-promise penalties and AI offers. Automatic post-war truces are visible in the country and military panels. The vassalage slice adds 30-day offers, 10% monthly tribute, automatic defense, independence wars, AI answers and typed LAN/UI commands. The country-list diplomacy panel projects opinions and alliances, sends alliance proposals and responses through host-validated commands, and supports eligible ruler/heir marriage proposals. Full two-player UI acceptance and M6 acceptance remain open.
The royal-marriage slice uses named adult characters and synthetic tests; the 1440 content currently has no female character or modeled existing spouse links. Historical marriage content is an M11 gap.
The war-participants prerequisite supports additional countries on either side of one war, with side-based armies, sieges, peace and pairwise truces.
M8 sea graph and strict v3 port validation are complete. The fleet/transport slice adds deterministic sea movement, port embarkation and landing, and an AI amphibious landing test. The naval combat slice resolves opposing fleets and transport losses. The blockade slice suppresses tax from unopposed wartime blockaded ports. Original instanced low-poly fleet models are implemented; attrition, construction and full AI L3 acceptance remain open.
The 1440 scenario currently starts with neutral opinions; historically supported positive opening relationships and their values belong to M11 content review. Until then, the alliance AI's favorable branch is covered by a synthetic scenario test.
M11 adds a sourced Duchy of Milan country slice on three full-cell and one 99.7%-overlap 1440 polygon match; lower-overlap border provinces remain unowned. The 8-country scenario passed a 10-year seed-1440 AI smoke; the wider European country and ruler set and M10 multi-seed soak remain incomplete.

M13 is the first post-beta milestone; colonization, trade, religion and later eras follow.
M10 has a yearly multi-seed headless soak report harness; L4 strategy, balance acceptance and the scheduled workflow remain open until M6–M9 mechanics and M11 beta content are ready.
Vertical slice = M2–M7 on the small test region (see DECISIONS D5), then expand to beta map in M11.
