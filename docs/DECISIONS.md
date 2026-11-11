# DECISIONS (log + open questions)

## Open
- D4 Windows x64 is the current test platform. The first PC has a verified Tauri launch and GPU measurement. The second player's GPU and FPS and a one-hour LAN session on two physical Windows PCs remain open. Same-PC runs verify loopback behavior.
- D8 Disconnect policy. ASSUMED: hold the country for a grace period, then the host may enable temporary AI. M5 exposes a host-selected 0–60 minute grace, initially 2 minutes. Resume tokens last for the running guest process and reserve its seat for the room lifetime. A permanent guest app exit requires recreating the room. This remains a prototype policy for review.
- D9 AI visibility. ASSUMED: AI sees the full state in beta, with no fog of war.
- D10 AI difficulty. ASSUMED: no cheats by default. Bonuses require explicit modifiers.
- D13 Horizontal wrap on the world map. ASSUMED: decide in M13.
- D14 Far-region countries as stub countries first. ASSUMED: yes.

## Decided
- D1 Time mode: real-time with pause. Each tick advances one day in the proleptic Gregorian calendar.
- D2 Time controls: either human player may pause or resume. Only the host sets speed.
- D0 Stack: Rust sim/net, Tauri 2, React/TS UI, three.js map. Fallback if WebView/WebGL fails: Godot.
- D3 Network: host-authoritative with deltas; lockstep deferred.
- D5 First vertical-slice region: Western Europe and the British Isles, with 5–8 countries. Slice target ~400–600 provinces (supersedes the ~100-province target, which was too coarse). Confirm density in M5b and record the measurements here.
- D6 Graphics: 3D terrain + low-poly instanced units, 2D UI; gameplay is province-graph based.
- D7 Scope: beta starts in 1440 and covers Europe, the Mediterranean and the Near East, with content to ~1500. Density follows MAP_SCALE.md. The world map follows the beta in M13.

- D11 AI lives in the `ai` crate, runs on the host, and acts through validated commands. It uses utility scoring, data-driven profiles, and an RNG derived from `(seed, tick, country_id)`.

- D12 A province is a settlement and its surroundings. The hierarchy is province → area → region (MAP_SCALE.md).

## Measurements (fill in)
| Date | Item | Result |
|------|------|--------|
| 2026-11-11 | First Windows PC, [PR #2](https://github.com/Zaywoo66/Game/pull/2) Tauri WebView | WebGL renderer/GPU: ANGLE (NVIDIA, NVIDIA GeForce RTX 4050 Laptop GPU (0x000028E1) Direct3D11 vs_5_0 ps_5_0, D3D11); version: WebGL 2.0 (OpenGL ES 3.0 Chromium) |
| 2026-11-11 | M1 synthetic tick benchmark (provisional) | Criterion median 243.27 ns/tick (95% CI 242.22–246.23 ns, 100 samples). `cargo bench --locked -p strategy-data --bench tick`, optimized bench profile, Rust 1.99.1 MSVC, Windows 11 Pro, Ryzen 7 7435HS. The fixture has 400 provinces and 8 AI-stub countries with one permanent modifier source each. This measures the M1 tick shell only. The full beta-map and gameplay benchmark is still pending. |
| 2026-11-11 | M2 Windows map benchmark | Tauri WebView2, RTX 4050 Laptop GPU via ANGLE/D3D11. Default 49-province overview with animated water, fullscreen drawing buffer 1920×1080, 2 s warm-up and 10 s frame sample: 60.9 FPS mean, p95 16.8 ms. App and descendant process working sets: 447.33 MiB second-half median, 449.82 MiB peak over 22 samples in 15.48 s. Summed working sets can include shared pages. Windows exposed only the discrete GPU, so integrated graphics performance remains unverified. |
| 2026-11-11 | M4 L1 autoplay on the 49-province test map | Seeds 1440, 1441 and 1442 ran 36,890 daily ticks each (101 years). Each run recorded one war in the opening decade, one treaty with territory ceded, all six countries still present and eight provinces in the largest country. Median host tick per run was 0.146, 0.076 and 0.078 ms in the Windows MSVC debug build. This measures the test slice. |
