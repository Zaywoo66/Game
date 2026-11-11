# NETWORK (crates/net)

## Model
Host-authoritative. Host runs sim. Client sends `Command`s, receives `Delta`s and periodic checksum. No lockstep in beta (see DECISIONS.md D3).

## Transport
`tokio` + TCP with Postcard 1.1.3 payloads and a big-endian unsigned 32-bit
length prefix. Client frames are bounded to 64 KiB and server frames to
8 MiB. Readers run dedicated tasks so cancellation cannot consume part of
the next frame. The handshake validates protocol version 1 and both map
identity fields before accepting a guest.

## Messages (v1)
- Client->Host: `Hello{version,name,map_version,map_hash,resume}`, `Confirm{resume}`, `Command{seq,command}`, `PauseRequest{seq,paused}`, `Ping{nonce}`, `Resync`.
- Host->Client: `Welcome{player_id}`, `Snapshot{state}` (join/resync), `Delta{tick,changes}`, `CommandRejected{seq,reason}`, `Checksum{tick,hash}`, `Pause{bool}`, `Speed{n}`, `Pong`.
- Either human player can pause or resume. Only the host sets speed (D2).
- Welcome includes a private 128-bit resume token. The guest confirms it
  after parsing its first snapshot. A disconnected, unconfirmed handshake
  releases the pending seat. A reconnect can reclaim an empty pending seat
  with its known token. A confirmed reservation requires the matching token.
- Commands receive `CommandAccepted{seq}` or `CommandRejected{seq,reason}`.
  The host remembers the last processed sequence for the guest slot and
  rejects duplicates. Reconnect retains the client's next sequence and
  installs current state. Commands are not replayed automatically.

## LAN discovery
- Host advertises `_game._tcp.local.` via mDNS (`mdns-sd`), TXT: room name, version, player count. Client menu lists discovered rooms. Fallback: manual IP:port entry (always keep this).
- Listen on `0.0.0.0:<port>`. Document the Windows Firewall prompt in README.
- Client-isolation on routers blocks LAN; note in README troubleshooting. Tailscale/ZeroTier work unchanged.

## Resync
The host sends a checksum every 30 daily ticks. Both sides hash the exact
canonical save bytes with SHA-256 after `strategy:lan-state:v1` and a NUL
byte. A mismatch logs both hashes and requests a fresh snapshot.

Network deltas contain a base sequence, new sequence and ordered byte edits
against the previous canonical save. The replica validates all ranges and
size bounds before replacing its bytes atomically. Equal-length changes
use sparse edit runs. Length-changing edits retain the shared prefix and
suffix. The guest loads the resulting snapshot into read-only simulation
types for queries and UI projection. It runs no ticks or AI.

Sequence gaps and a lagged outbound receiver trigger a fresh snapshot.
Every full snapshot installation increments a durable client revision,
including same-sequence corrections, so the UI refreshes even when a
transient resync phase is coalesced. Initial state, join and resync use full
snapshots. Ordinary gameplay sends deltas.

## Reconnect
The host chooses a grace period from 0 to 60 minutes in the lobby. The UI
starts at 2 minutes. A confirmed guest slot remains reserved for the room
lifetime. Its token lives in the guest process and automatically resumes
after a transient connection loss. Closing and reopening the guest app
requires recreating the room. A reconnecting guest receives a full snapshot.

## Tests
Loopback integration tests: host + 1 client in one process, scripted commands, assert identical hashes after 10k ticks.

## Lobby and controllers
- The host chooses which countries are human-controlled. All others use AI. Only the host runs AI.
- Messages: `Lobby{countries:[{tag,controller}]}`, `ClaimCountry{tag}`, and `SetController{tag,controller}` (host only).
- When a player disconnects, their country is held for a grace period. The host may then enable temporary AI. Reconnecting restores human control (D8).
- Balanced, cautious and aggressive presets load tuning from
  `data/ai/lan_presets.json`. Country weights remain scenario data.
