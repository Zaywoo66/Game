# NETWORK (crates/net)

## Model
Host-authoritative. Host runs sim. Client sends `Command`s, receives `Delta`s and periodic checksum. No lockstep in beta (see DECISIONS.md D3).

## Transport
`tokio` + TCP, length-prefixed binary messages (`serde` + `bincode` or `postcard`). Protocol version in handshake; mismatch => reject with message.

## Messages (v1)
- Client->Host: `Hello{version,name}`, `Command{seq,cmd}`, `PauseRequest`, `SpeedVote{n}`, `Ping`.
- Host->Client: `Welcome{player_id}`, `Snapshot{state}` (join/resync), `Delta{tick,changes}`, `CommandRejected{seq,reason}`, `Checksum{tick,hash}`, `Pause{bool}`, `Speed{n}`, `Pong`.
- Pause/speed: either player can pause; speed changes require both votes (or host decides — see D2).

## LAN discovery
- Host advertises `_game._tcp.local.` via mDNS (`mdns-sd`), TXT: room name, version, player count. Client menu lists discovered rooms. Fallback: manual IP:port entry (always keep this).
- Listen on `0.0.0.0:<port>`. Document the Windows Firewall prompt in README.
- Client-isolation on routers blocks LAN; note in README troubleshooting. Tailscale/ZeroTier work unchanged.

## Resync
Client computes state hash each N ticks; mismatch with host `Checksum` => request `Snapshot`. Log both hashes.

## Reconnect
Host keeps player slot for X minutes; reconnecting client receives `Snapshot`.

## Tests
Loopback integration tests: host + 1 client in one process, scripted commands, assert identical hashes after 10k ticks.

## Lobby and controllers
- The host chooses which countries are human-controlled. All others use AI. Only the host runs AI.
- Messages: `Lobby{countries:[{tag,controller}]}`, `ClaimCountry{tag}`, and `SetController{tag,controller}` (host only).
- When a player disconnects, their country is held for a grace period. The host may then enable temporary AI. Reconnecting restores human control (D8).
