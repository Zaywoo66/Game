# Changelog

## Unreleased

- Added save schema version 1. Loading rejects mismatched map versions or hashes.
- M3 added save schema v2 for countries, character succession, budgets and AI profiles. V1 remains readable with its matching map.
- M4 adds map schema and map version 2 for weighted edges, and save schema v3 for armies, wars and treaties. Older saves still require their original map identity.
- M6 opinion foundation added save schema v4 for directed country opinions. Older saves retain their original encoding and hash.
- M6 alliances add save schema v5 for alliance offers, active treaties and realliance cooldowns. The LAN command protocol advances to v3; players must use the same build.
- M6 war-participants prerequisite adds save schema v6 for side supporters and pairwise coalition truces. Older saves keep their original encoding. LAN protocol advances to v4; both players must update together.
- M6 royal marriages add character schema v2, save schema v7 and LAN protocol v5. V1–v6 saves load with unknown character sex; those characters cannot create new marriages because sex cannot be inferred from old saves. Both players must update together.
- M7 event runtime adds save schema v8 and LAN protocol v6. V8 snapshots include event definitions, pending choices, resolution history and modifier-source sequence; v1–v7 saves remain readable without events. Both players must update together.
- M6 guarantees add save schema v9 and LAN protocol v7. V9 snapshots preserve active guarantees; older saves remain readable without them. Both players must update together.
- M8 fleet transport adds save schema v10 and LAN protocol v8. V10 snapshots preserve fleets, sea routes and embarked army IDs; v1–v9 saves remain readable without navy. Both players must update together.
- M9 research adds save schema v11 and LAN protocol v9. V11 records point balances, technology definitions, and permanent modifier sources. Both players must update together; older saves remain readable without research state.

## 0.1.0 — Foundation

- Created the Rust workspace, Tauri shell, React/TypeScript UI, and WebGL cube page.
- Added CI and project documentation.

