## Goal
Establish the M0 application and repository foundation.

## Milestone
M0 Foundation (in progress pending Windows app launch, CI, and branch protection).

## What changed
- Added Cargo workspace with empty `sim`, `net`, and `data` crates and a Tauri 2 shell.
- Added React/TypeScript UI with a three.js spinning cube and startup WebGL renderer/GPU console log.
- Added GitHub Actions checks, a basic JSON syntax linter, copied project docs, and a changelog.
- Registered the original placeholder app icon in `docs/ASSETS.md`.

## Pinned versions
| Component | Version |
|---|---|
| Rust edition | 2021 |
| Rust toolchain | 1.99.1 |
| Tauri / Tauri Build / Tauri CLI | 2.12.5 / 2.7.4 / 2.12.4 |
| React / React DOM | 18.3.1 |
| Vite | 5.4.10 |
| three.js | 0.169.0 |

## How verified
- `cargo fmt --check`: passed after formatting.
- `cargo clippy -p strategy-sim -p strategy-net -p strategy-data --all-targets -- -D warnings`: passed.
- `cargo test -p strategy-sim -p strategy-net -p strategy-data`: passed (0 tests; no mechanics exist in M0).
- `npm run lint` and `npm run build` from `ui`: passed.
- `python tools/lint_data.py`: passed; 0 JSON files are present in M0.
- Full workspace Clippy/test: blocked on this machine's Windows GNU toolchain (`dlltool.exe` missing; bundled dlltool cannot launch a subprocess). CI targets Windows MSVC.

## Docs updated?
`docs/ROADMAP.md` marks M0 in progress. `docs/ASSETS.md` registers the icon. `CHANGELOG.md` records the scaffold.

## Risks, conflicts, and proposed doc fixes
- The supplied checkout had root `ROADMAP.md` and `repo.zip`, but no `docs/` directory. The archive contained the canonical docs, now copied into `docs/`. Proposed fix: distribute or clone the repository with `docs/` already present and remove the stale root roadmap.
- M0 requests CI green on `main`, branch protection, and app launches on both players' Windows systems. These require pushing the branch, GitHub access, and running the app on both PCs. M0 remains in progress until those checks are observed. Proposed doc fix: split M0 acceptance into repository checks and per-player launch evidence.
- `docs/GIT_WORKFLOW.md` calls for a data linter with schemas and cross references, plus a determinism test. M0 has no data content or simulation mechanics, so the current linter checks JSON syntax only and the determinism test is deferred. Proposed doc fix: state the milestone when schema, cross-reference, and determinism gates first apply.
- The current local Windows toolchain is GNU, while Tauri's Windows prerequisite is MSVC and Microsoft C++ Build Tools. The workflow uses MSVC. Proposed doc fix: add a Windows prerequisite line to M0.
