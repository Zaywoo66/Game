# DECISIONS (log + open questions)

## Open
- D1 Real-time with pause vs turn-based. ASSUMED: real-time with pause.
- D2 Who controls speed/pause (host only / either pauses, both vote speed). ASSUMED: either pauses, host sets speed.
- D4 Target OS and WebGL verification. OPEN. Both players use Windows. First PC passed in Tauri. Second PC launch and renderer string are pending.
- D5 First vertical-slice region. PROPOSED: Western Europe + British Isles, ~100 provinces, 5–8 countries.

- D8 Disconnect policy. ASSUMED: hold the country for a grace period, then the host may enable temporary AI.
- D9 AI visibility. ASSUMED: AI sees the full state in beta, with no fog of war.
- D10 AI difficulty. ASSUMED: no cheats by default. Bonuses require explicit modifiers.

## Decided
- D0 Stack: Rust sim/net, Tauri 2, React/TS UI, three.js map. Fallback if WebView/WebGL fails: Godot.
- D3 Network: host-authoritative with deltas; lockstep deferred.
- D6 Graphics: 3D terrain + low-poly instanced units, 2D UI; gameplay is province-graph based.
- D7 Scope: beta = start 1440, Europe+Mediterranean+Near East (~300–500 provinces), content to ~1500. Later eras and the rest of the world = separate content releases.

- D11 AI lives in the `ai` crate, runs on the host, and acts through validated commands. It uses utility scoring, data-driven profiles, and an RNG derived from `(seed, tick, country_id)`.

## Measurements (fill in)
| Date | Item | Result |
|------|------|--------|
| 2026-11-11 | First Windows PC, [PR #2](https://github.com/Zaywoo66/Game/pull/2) Tauri WebView | WebGL renderer/GPU: ANGLE (NVIDIA, NVIDIA GeForce RTX 4050 Laptop GPU (0x000028E1) Direct3D11 vs_5_0 ps_5_0, D3D11); version: WebGL 2.0 (OpenGL ES 3.0 Chromium) |
