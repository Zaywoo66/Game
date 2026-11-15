# M1 Simulation core
Docs: SIM_RULES.md, DATA_FORMATS.md

## Tasks
- Date/tick system, speeds 1-5, pause
- Province graph structure + loader from data crate
- Fixed-point type, seeded RNG, id newtypes
- Modifier system, trigger and effect evaluation
- Command/apply/Delta plumbing; save/load of GameState (versioned)

## Acceptance
- Determinism test: same seed+commands => identical state hash after 10k ticks
- Save -> load -> continue equals uninterrupted run
- Tick benchmark recorded in DECISIONS.md

## AI slice (required)
Docs: AI.md
- Country `controller: Human(player_id) | Ai(profile_key)` in GameState, host-only command to change it
- Read-only query helpers in sim for AI (e.g. country strength, neighbors, legal-command checks) — no mutation
- AI RNG derivation from (seed, tick, country_id); tick order includes the AI step (see SIM_RULES.md)
- Empty `crates/ai` crate with `decide` stub returning no commands; CI builds it
- Acceptance add-on: a run with all countries set to Ai(stub) stays deterministic
