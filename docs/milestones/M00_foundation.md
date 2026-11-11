# M0 Foundation
Docs: GIT_WORKFLOW.md, ARCHITECTURE.md

## Tasks
- Cargo workspace (sim, net, data), Tauri 2 app, React/TS UI scaffold
- CI workflow (fmt, clippy, test, UI build) and branch protection
- three.js page inside Tauri rendering a spinning cube
- Copy these docs into the repo, create CHANGELOG.md

## Windows setup
- Install the Rust MSVC toolchain for the PC's architecture (`x86_64-pc-windows-msvc` for the CI build).
- Install Visual Studio Build Tools with the **Desktop development with C++** workload.
- Install Microsoft Edge WebView2 Runtime if it is missing.
- Build with Node.js and npm, or download and extract the Windows x64 artifact from a successful GitHub Actions run. The artifact includes the executable and requires WebView2 Runtime on the target PC.

## Acceptance

### Repository checks
- The foundation CI checks pass on the PR and on `main` after merge.
- `main` requires a PR, human review, and passing CI.
- CI publishes the Windows x64 smoke test executable.

### Launch checks
- Launch `strategy-app.exe` from the extracted artifact on the available Windows PC.
- Confirm the Tauri window opens and the cube rotates. Record the `WebGL renderer/GPU:` line displayed in the window and logged to the WebView console.
- The owner accepted the verified single-PC launch for M0. A second Windows PC is unavailable (D4).
- If WebGL fails on a target OS: record in DECISIONS.md and stop to decide on fallback (Godot)
