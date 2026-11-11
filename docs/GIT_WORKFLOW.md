# GIT_WORKFLOW

## Branches
- `main`: always green. Protected: PR + passing CI required.
- Short-lived: `feat/<module>-<topic>`, `fix/<module>-<topic>`, `data/<topic>`, `art/<topic>`, `docs/<topic>`, `tools/<topic>`. Max 1–2 days of work each.
- Agent works only on its own branch; a human reviews every PR before merge.

## Commits
Conventional style: `feat(sim): army pathfinding`, `fix(net): resync on checksum mismatch`.

## CI (GitHub Actions)
1. `cargo fmt --check` 2. `cargo clippy --all-targets -- -D warnings` 3. `cargo test --workspace` 4. UI: install, lint, typecheck, build 5. Data linter (schemas + cross-refs) 6. Determinism test.
Release workflow on tag `v*`: build Windows/macOS/Linux bundles via Tauri, upload to GitHub Releases.

## PR template
- Goal (1 line) / Milestone / What changed / How verified (commands + results) / Docs updated? / Risks.

## Versions
`v0.x.0` per milestone group. Save-game and data `schema_version` bumps recorded in CHANGELOG.
