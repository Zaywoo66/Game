# DECISIONS (log + open questions)

## Open
- D1 Real-time with pause vs turn-based. ASSUMED: real-time with pause.
- D2 Who controls speed/pause (host only / either pauses, both vote speed). ASSUMED: either pauses, host sets speed.
- D4 Target OS for both players (affects WebGL/WebView risk, esp. Linux). UNKNOWN.
- D5 First vertical-slice region. PROPOSED: Western Europe + British Isles, ~100 provinces, 5–8 countries.

## Decided
- D0 Stack: Rust sim/net, Tauri 2, React/TS UI, three.js map. Fallback if WebView/WebGL fails: Godot.
- D3 Network: host-authoritative with deltas; lockstep deferred.
- D6 Graphics: 3D terrain + low-poly instanced units, 2D UI; gameplay is province-graph based.
- D7 Scope: beta = start 1440, Europe+Mediterranean+Near East (~300–500 provinces), content to ~1500. Later eras and the rest of the world = separate content releases.

## Measurements (fill in)
| Date | Item | Result |
|------|------|--------|
