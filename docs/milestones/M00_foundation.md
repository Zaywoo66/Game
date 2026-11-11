# M0 Foundation
Docs: GIT_WORKFLOW.md, ARCHITECTURE.md

## Tasks
- Cargo workspace (sim, net, data), Tauri 2 app, React/TS UI scaffold
- CI workflow (fmt, clippy, test, UI build) and branch protection
- three.js page inside Tauri rendering a spinning cube
- Copy these docs into the repo, create CHANGELOG.md

## Acceptance
- CI green on main; app launches on both players' OS; WebGL context created (log GPU/renderer)
- If WebGL fails on a target OS: record in DECISIONS.md and stop to decide on fallback (Godot)
