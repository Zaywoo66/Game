# GIT_WORKFLOW

## Branches
- `main`: always green. Protected: PR + passing CI required.
- Short-lived: `feat/<module>-<topic>`, `fix/<module>-<topic>`, `data/<topic>`, `art/<topic>`, `docs/<topic>`, `tools/<topic>`. Max 1–2 days of work each.
- Agent works only on its own branch; a human reviews every PR before merge.
- Keep an actively developed PR in draft while batching related implementation work. CI is skipped for draft PRs and runs when the PR is marked ready; do not open a separate PR or run the full suite after each small edit. Run focused checks only to diagnose a concrete issue. Before marking ready, run the required suite once and report exact results.

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
| Fast AI decision and battle tests | M4 |
| TCP loopback, checksum/resync, reconnect and authority tests | M5 |

Long-duration acceptance runs are opt-in, not part of every PR. The default
workspace test command skips tests marked `#[ignore]`; run those tests only
for release acceptance or changes to the affected subsystem. This keeps the
regular feedback loop short while retaining the soak checks for milestone
acceptance.

Opt-in commands:

- 101-year deterministic AI replay: `cargo test -p strategy-app --test autoplay the_1440_scenario_replays_for_101_years_under_ai_control -- --ignored`
- Three-seed 10-year AI smoke: `cargo test -p strategy-app --test military_smoke three_seeds_complete_ten_years_and_ai_wars_end_in_treaties -- --ignored`
- 10,000-tick TCP AI replay: `cargo test -p strategy-app --test lan_replay lan_guest_matches_host_after_ten_thousand_ticks_with_ai -- --ignored`
- 100-year/five-seed M10 report: `cargo run --locked -p strategy-app --bin ai-soak -- 100 1440 1441 1442 1443 1444`

Native GPU/FPS/RAM and two-PC LAN checks are manual milestone/release evidence;
run them at the relevant milestone/release acceptance point, not after each
rendering or network edit. Record a development smoke result separately from
formal acceptance; only repeat early when a concrete change plausibly invalidates
the existing measurement.

CI caches Cargo dependencies and compiled dependency artifacts after installing
the pinned Rust toolchain. A cache miss still runs every required check. The
cache key follows the toolchain and Cargo manifests/lockfile, so changes to
dependencies trigger the necessary rebuilds.

Release workflow on tag `v*`: build Windows/macOS/Linux bundles via Tauri, upload to GitHub Releases.

## PR template
- Goal (1 line) / Milestone / What changed / How verified (commands + results) / Docs updated? / Risks.

## Versions
`v0.x.0` per milestone group. Save-game and data `schema_version` bumps recorded in CHANGELOG.
