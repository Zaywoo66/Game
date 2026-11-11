# M7 Events and RP
Docs: DATA_FORMATS.md, SIM_RULES.md

Progress: event YAML contract and typed loader, deterministic firing and choice
commands, AI weighted responses, and a three-event generic 1440 starter pack
with English localization are in progress. The 20–40 event content target,
AI-only content run, and multiplayer acceptance remain open. The pending-event
popup presents localized choices for the local player and sends each selection
to the host; the browser test covers the command and host-driven dismissal.

## Tasks
- Event engine: triggers, mean time to happen, options, effects, popup UI
- Ruler traits and life events; 20-40 historical/random events in data files
- Localization keys for all text

## Acceptance
- Events fire deterministically by seed; data linter rejects unknown triggers/effects
- Both players see and resolve their own events in multiplayer

## AI slice (required)
Docs: AI.md
- **AI L3 (events)**: AI picks event options by `ai_weight` and profile; weights required for every option in data
- Data linter fails if an event option lacks `ai_weight`
- Acceptance add-on: AI-only run fires events without stalling on pending choices
