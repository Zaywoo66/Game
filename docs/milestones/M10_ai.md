# M10 AI — strategic layer, balance, tuning (L4)
Docs: AI.md, SIM_RULES.md
Prerequisite: L0–L3 already delivered in M3–M9. This milestone does not introduce AI from scratch.

Progress: a headless multi-seed soak report harness records yearly state hashes,
country counts, largest land share and local tick-time percentiles. This is a
measurement tool; the five-seed acceptance run and L4 behavior remain open.

## Tasks
- Threat model and balance of power: AI reacts to dominant neighbors, forms coalitions
- Alliance coordination: call allies, honor/break treaties with consequences
- Long-term goals per profile (expansion targets, core regions, naval ambitions)
- Performance: operation caps, staggering, benchmark with the full beta map and all AI countries
- Difficulty/aggressiveness presets (explicit modifiers only)
- Soak test workflow (100+ years, multiple seeds), results table in DECISIONS.md
- Fix pathological behaviors found by soak (suicide wars, stuck armies/fleets, bankruptcy spirals, runaway snowballing)

## Acceptance
- 100-year AI-only run across 5 seeds: no panic, no invariant violation, tick time within budget
- Distribution sanity: no single country owns >40% of the map before year 1600 in a majority of seeds; no mass extinction of major powers
- AI never bypasses validation; every decision has a reason code
- Determinism test with AI passes (state hash identical across runs and save/load)
