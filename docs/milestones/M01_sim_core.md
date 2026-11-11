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
