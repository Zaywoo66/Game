# M10 AI — strategic layer, balance, tuning (L4)
Docs: AI.md, SIM_RULES.md
Prerequisite: L0–L3 already delivered in M3–M9. This milestone does not introduce AI from scratch.

Progress: a headless multi-seed soak report harness records yearly state hashes,
country counts, largest land share and local tick-time percentiles. War-target
scoring now estimates the defensive coalition that would actually join:
the target, available guarantors, and its available overlord, excluding
supporters already at war or allied with the attacker. Calls to allies are now
implemented; treaty coordination beyond those offers, the five-seed acceptance
run, remaining L4 goals and soak fixes remain open.

Call-to-arms is implemented as a separate slice: both war leaders invite their
peaceful allies for 30 game days, AI answers using `alliance_min_opinion`,
human players choose explicitly, and either answer closes any competing
invitation for that war. The host-validated commands and pending offers are
included in LAN protocol v15 and save schema v17. This does not close M10's
multi-seed soak or remaining L4 acceptance.

## Tasks
- Threat model and balance of power: AI reacts to dominant neighbors, forms coalitions
- Alliance coordination: call allies, honor/break treaties with consequences
- Long-term goals per profile (expansion targets, core regions, naval ambitions)
- Performance: operation caps, staggering, benchmark with the full beta map and all AI countries
- Difficulty/aggressiveness presets (explicit modifiers only)
- Soak test workflow (100+ years, multiple seeds), results table in DECISIONS.md
- Fix pathological behaviors found by soak (suicide wars, stuck armies/fleets, bankruptcy spirals, runaway snowballing)

## Acceptance (release soak; not a per-PR gate)
- 100-year AI-only run across 5 seeds: no panic, no invariant violation, tick time within budget
- Distribution sanity: no single country owns >40% of the map before year 1600 in a majority of seeds; no mass extinction of major powers
- AI never bypasses validation; every decision has a reason code
- Determinism test with AI passes (state hash identical across runs and save/load)

Fast AI unit and command-validation tests run in ordinary CI. The 101-year
single-seed replay, three-seed 10-year smoke and 100-year/five-seed soak are
opt-in release or AI-balance checks; see GIT_WORKFLOW.md. Do not rerun them for
unrelated data or UI edits.
