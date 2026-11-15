# M5 handoff

The playable test scenario begins on 1 January 1440. It has six countries
and 49 provinces in Western Europe and the British Isles. M5 is the end
of this implementation scope. The roadmap lists the next milestone.

## Change history

| Milestone | PR | Scope |
|---|---|---|
| M2 | [#12](https://github.com/Zaywoo66/Game/pull/12) | Reproducible map, three.js picking and owner lookup, native WebGL benchmark |
| M3 | [#13](https://github.com/Zaywoo66/Game/pull/13) | Fiscal economy, characters and succession, L0 AI and country/ruler panels |
| M4 | [#14](https://github.com/Zaywoo66/Game/pull/14) | Armies, weighted A*, combat, siege, occupation, treaties, L1 AI and instanced infantry |

## Where to work

- `crates/sim` owns deterministic time, state, command validation and
  deltas. It uses fixed-point thousandths, a seeded RNG and ordered
  collections. The sim has no I/O.
- `crates/data` validates scenario content and canonical saves. Current
  map metadata and map identity are version 2. Games with military state
  use save v3. Legacy saves require the original map identity.
- `crates/ai` emits commands and reasons for the host. L0 covers budgets
  and heirs. L1 handles land wars. `ai-smoke` runs these decisions without
  a window.
- `crates/net` implements TCP/Postcard v1, mDNS and canonical byte
  replication. `app/src/lan` owns the host and guest lifecycles.
  Only the host advances the simulation. The guest reads reconstructed
  state and sends commands.
- `app/src/session.rs` projects the authoritative snapshot and updates
  into Tauri. `ui/src/game` contains the lobby and gameplay panels.
  `ui/src/map` renders province ownership, occupation and army positions.
- `tools/map` regenerates the source-attributed map. `tools/models`
  reproduces the original low-poly infantry GLB with Blender.

## Verify

Use the Windows MSVC setup and exact commands in the root README.
The CI job `foundation` builds the Windows artifact
`strategy-windows-x64`. The browser tests use a lower software-renderer
resolution while retaining CSS layout and actual rendering assertions.
Failed runs publish `browser-failure-diagnostics`.

- M3 ran 101 AI-controlled years with exact save/replay hashes and a
  reload at year 50.
- M4 runs 10 years on three seeds in CI, and each seed reaches territorial
  peace. The scripted test also checks path cost, battle, retreat, siege,
  occupation and cession. Local 101-year runs are recorded in DECISIONS.md.
- M5 drives a real TCP host and client for 10,000 ticks. It checks
  authoritative AI outcomes and identical final save bytes. The failure
  tests cover checksum resync, same-sequence correction, reconnect,
  controller authority, temporary AI takeover and abandoned handshakes.

## Limits to retain

- The owner has one Windows PC. Cross-PC LAN compatibility and
  integrated-GPU performance remain unmeasured. The native map benchmark
  used the RTX 4050 laptop GPU through ANGLE/D3D11.
- D8 is still a prototype policy. The host selects a 0–60 minute grace
  period, with a two-minute UI default. Resume tokens remain in the
  running guest process. Recreating a room releases a permanently
  disconnected guest seat.
- M4 supports one active war per country and one non-capital territorial
  goal per war. The test map and authored character roster are a small
  playable slice. The current data stays subject to map identity and
  schema checks.
