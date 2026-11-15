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

## AI slice (required)
Docs: AI.md
- AI runs on host only; clients receive AI effects as ordinary deltas
- Lobby: host chooses which countries are human-controlled; the rest are AI. AI aggressiveness preset selectable
- Disconnect handling per DECISIONS D8: country held, then optional temporary AI takeover by host; reconnect returns control
- Acceptance add-on: 2 humans + AI countries for 1 hour, no checksum mismatches, AI actions visible to both clients
