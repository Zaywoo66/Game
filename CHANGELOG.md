# Changelog

## Unreleased

- Added save schema version 1. Loading rejects mismatched map versions or hashes.
- M3 added save schema v2 for countries, character succession, budgets and AI profiles. V1 remains readable with its matching map.
- M4 adds map schema and map version 2 for weighted edges, and save schema v3 for armies, wars and treaties. Older saves still require their original map identity.
- M6 opinion foundation added save schema v4 for directed country opinions. Older saves retain their original encoding and hash.
- M6 alliances add save schema v5 for alliance offers, active treaties and realliance cooldowns. The LAN command protocol advances to v3; players must use the same build.

## 0.1.0 — Foundation

- Created the Rust workspace, Tauri shell, React/TypeScript UI, and WebGL cube page.
- Added CI and project documentation.

