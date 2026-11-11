# ARCHITECTURE

## Layout
```
/crates/sim    pure simulation (state, ticks, wars, economy, events)
/crates/net    protocol + transport + LAN discovery
/crates/data   load + validate data files -> sim types
/crates/ai     bot decision logic (reads sim state, emits Commands)
/app           Tauri shell (Rust): hosts sim/net, exposes IPC to UI
/ui            React + TS: windows, HUD, event popups
/ui/map        three.js: terrain, shaders, instanced units
/data          game content (JSON/YAML)
/tools         map generation, model export scripts (Python/Blender)
/docs          these documents
```
Dependency direction: `app -> net, data, sim, ai`; `ai -> sim`; `net -> sim` (types only); `data -> sim`; `sim -> nothing`.

## Process model
- Host runs `sim` authoritatively inside the Tauri app. Client app connects to host over TCP and only sends commands.
- UI (WebView) talks to the local Rust side via Tauri IPC. IPC carries per-tick DELTAS, never full state (full snapshot only on load/join/resync).
- Time: 1 tick = 1 game day. Speeds 1–5 + pause. Default: real-time with pause (see DECISIONS.md D1).

## Core abstractions (sim)
- `GameState`: single serializable struct; all mutation via `apply(Command)` and `tick()`.
- `Command`: player intent (MoveArmy, DeclareWar, SendProposal, ChooseEventOption...). Validated by sim, rejected with a reason code.
- `Delta`: list of changes produced by a tick for the UI.
- Modifier system: all bonuses = `key -> value` modifiers from techs, ideas, traits, laws, events; effective value is an aggregation. See SIM_RULES.md.
- Triggers/Effects: shared condition/effect language used by events, ideas, diplomacy availability, AI.

## AI
- The host runs deterministic AI. AI reads simulation state and acts through the same validated commands as a human player.
- Countries store a controller, either `Human(player_id)` or `Ai(profile_key)`. See [AI.md](AI.md).

## Map model
- Provinces form a graph (land, sea, lake). Edges carry cost/type (river, strait, port link).
- Raster map: pixel color = province ID. Rendering uses lookup textures (province -> owner color). Click picking reads CPU copy of the raster.

## Performance budget (targets, measure and record in docs/DECISIONS.md)
- 60 FPS on integrated GPU at 1080p; total RAM < 1 GB in beta; tick for full beta map < 5 ms median.
