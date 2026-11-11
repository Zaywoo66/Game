# M5 LAN multiplayer
Docs: NETWORK.md, ARCHITECTURE.md

## Tasks
- Protocol v2, host-authoritative server in app, client mode
- mDNS discovery + manual IP entry; room lobby UI
- Deltas, checksum, resync, pause/speed rules, reconnect
- Loopback integration tests

## Acceptance
- Two physical Windows PCs play for one hour on the same LAN with zero unexplained checksum mismatches and no crash. Use [the acceptance checklist](../LAN_ACCEPTANCE.md).
- Record the second GPU, FPS, discovery result and connection details in D4.
- Firewall and router-isolation troubleshooting in README

## AI slice (required)
Docs: AI.md
- AI runs on host only; clients receive AI effects as ordinary deltas
- Lobby: host chooses which countries are human-controlled; the rest are AI. AI aggressiveness preset selectable
- Disconnect handling per DECISIONS D8: country held, then optional temporary AI takeover by host; reconnect returns control
- Acceptance add-on: both human seats and AI countries active for one hour, zero checksum mismatches, AI actions visible to both instances

## Automated verification

`cargo test --locked -p strategy-app --test lan_replay` drives a real
TCP host and guest for 10,000 ticks. It checks guest pause authority,
host-only speed, AI outcomes, checksum counts and exact final save bytes.
Separate tests exercise corrupt-checksum resync, a same-sequence corrected
snapshot, reconnect, held country and temporary AI, and a guest that
abandons its handshake. The UI suite verifies both lobby flows and
refreshes the authoritative snapshot after reconnection.
