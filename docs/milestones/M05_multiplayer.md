# M5 LAN multiplayer
Docs: NETWORK.md, ARCHITECTURE.md

## Tasks
- Protocol v1, host-authoritative server in app, client mode
- mDNS discovery + manual IP entry; room lobby UI
- Deltas, checksum, resync, pause/speed rules, reconnect
- Loopback integration tests

## Acceptance
- Two machines on one LAN play 1 hour with zero checksum mismatches
- Firewall and router-isolation troubleshooting written in README
