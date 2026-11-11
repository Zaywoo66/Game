# AGENTS.md — read this first (keep under 100 lines)

Project: LAN-playable grand strategy game (CK3 rulers + EU4 provinces/diplomacy + HOI4-style armies).
Players: 2 humans (host + friend); every other country is AI-controlled. Start date 1440, beta content up to ~1500, mechanics designed up to 1850.
Stack: Rust (`sim`, `net`, `data`) + Tauri 2 + React/TS UI + three.js map.

## How to work (context-saving protocol)
1. Read ONLY this file + `docs/ROADMAP.md` (index) + the ONE milestone file for the current task.
2. Open other docs only when the task touches them (table below). Never read all docs.
3. Do not read large data files (`provinces.png`, `data/**` dumps). Use schemas in `docs/DATA_FORMATS.md`.
4. One task = one branch = one PR. Do not refactor unrelated code.
5. Update `docs/ROADMAP.md` status and relevant docs in the same PR.

## Which doc to open
| Touching...                         | Read                      |
|-------------------------------------|---------------------------|
| crate layout, boundaries            | docs/ARCHITECTURE.md      |
| anything in `crates/sim`            | docs/SIM_RULES.md         |
| JSON/YAML content, schemas          | docs/DATA_FORMATS.md      |
| map generation, rendering           | docs/MAP_PIPELINE.md      |
| province density, hierarchy, world readiness | docs/MAP_SCALE.md |
| multiplayer, protocol, LAN          | docs/NETWORK.md           |
| models, icons, textures, licenses   | docs/ASSETS.md            |
| branches, CI, PRs, releases         | docs/GIT_WORKFLOW.md      |
| writing/accepting a task            | docs/TASK_TEMPLATE.md     |
| open questions, past decisions      | docs/DECISIONS.md         |
| AI / bots, country controllers      | docs/AI.md                |

## Hard rules (non-negotiable)
- `crates/sim`: no `f32/f64`, no system time, no randomness outside the seeded RNG, no unordered `HashMap` iteration (use `BTreeMap`/`IndexMap`), no I/O, no UI/net imports.
- Content (countries, techs, events, ideas) lives in data files, never hardcoded.
- Every mechanic ships with tests. No tests = task not accepted.
- UI never mutates game state; it sends commands to the host.
- No asset enters the repo without a license entry in `docs/ASSETS.md`.
- `main` must always build and pass CI. No direct commits to `main`.
- If a requirement is ambiguous or conflicts with these docs: stop, state the conflict in the PR description, do not guess.

- AI acts only through `Command`s validated like human commands; AI code lives in `crates/ai` and never mutates state directly. A mechanic without AI support is not accepted.

## Definition of done
`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, UI build, data lint all green; docs updated; PR description lists what changed and how it was verified.
