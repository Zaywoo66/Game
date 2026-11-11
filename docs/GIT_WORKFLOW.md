# GIT_WORKFLOW

## Branches
- `main`: always green. Protected: PR + passing CI required.
- Short-lived: `feat/<module>-<topic>`, `fix/<module>-<topic>`, `data/<topic>`, `art/<topic>`, `docs/<topic>`, `tools/<topic>`. Max 1–2 days of work each.
- Agent works only on its own branch; a human reviews every PR before merge.

## Commits
Conventional style: `feat(sim): army pathfinding`, `fix(net): resync on checksum mismatch`.

## CI (GitHub Actions)
| CI gate | First enabled |
|---|---|
| `cargo fmt --check` | M0 |
| `cargo clippy --locked --all-targets -- -D warnings` | M0 |
| `cargo test --locked --workspace` | M0 |
| UI install, lint, typecheck, and build | M0 |
| Data lint command, JSON syntax, and invalid fixture test | M0 |
| Data schema and cross-reference checks | M1 |
| Determinism test | M1 |
| Map validation | M2 |
| AI smoke test (10 game years, 3 seeds, `strategy-app` integration test `military_smoke`) | M4 |
| TCP loopback, checksum/resync, reconnect and authority (`strategy-app` integration test `lan_replay`) | M5 |

From M10, run the AI soak test through a scheduled or manual workflow.

Release workflow on tag `v*`: build Windows/macOS/Linux bundles via Tauri, upload to GitHub Releases.

## PR template
- Goal (1 line) / Milestone / What changed / How verified (commands + results) / Docs updated? / Risks.

## Versions
`v0.x.0` per milestone group. Save-game and data `schema_version` bumps recorded in CHANGELOG.
