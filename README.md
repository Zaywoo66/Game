# Strategy

Windows LAN grand-strategy prototype. The current test scenario starts in
1440 with six countries on a 49-province Western Europe map.

## Setup

Install Rust 1.99.1 with the MSVC toolchain and the rustfmt and Clippy
components, Node.js 24.14.1, Visual Studio Build Tools with the Desktop
development with C++ workload and a Windows SDK, and WebView2 Runtime.
Python 3.12 is used by the data and map checks.

Run these commands from the repository root in PowerShell.

```powershell
rustup override set 1.99.1-x86_64-pc-windows-msvc
rustup component add rustfmt clippy
npm --prefix ui ci
npm --prefix ui run build
cargo build --locked -p strategy-app --features custom-protocol --bin strategy-app
.\target\debug\strategy-app.exe
```

The CI artifact `strategy-windows-x64` contains the same standalone
executable. The UI, map, model and starting scenario are embedded at build
time. WebView2 Runtime is required on the receiving PC.

## Local checks

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --workspace
npm --prefix ui run lint
npm --prefix ui run typecheck
npm --prefix ui run build
npm --prefix ui exec -- playwright install chromium
npm --prefix ui run test:map
py -3.12 -m pip install -r tools/requirements.txt -r tools/map/requirements.txt
py -3.12 -B tools/lint_data.py
py -3.12 -B -m unittest discover -s tools/tests
py -3.12 -B -m unittest discover -s tools/map/tests
py -3.12 -B tools/map/validate.py --region western_europe
cargo run --locked -p strategy-data --bin lint-provinces -- data/map/western_europe/provinces.json
cargo run --locked -p strategy-data --bin lint-scenario -- data/scenarios/1440.json data/map/western_europe/provinces.json
```

For deterministic AI autoplay, the first argument is the number of daily
ticks and subsequent arguments are seeds.

```powershell
cargo run --locked -p strategy-app --bin ai-smoke -- 3653 1440 1441 1442
```

## LAN play

Open LAN in the app. The host creates a room, chooses two different human
countries, an AI aggressiveness preset and a disconnect grace period.
The remaining countries run on the host's AI. A new LAN room starts paused.
Either human can pause or resume. The host controls speed.

The guest can scan for rooms or enter the host's IPv4 address and TCP port.
The default port is 47620. Discovered rooms can show multiple endpoints
when the host has several adapters. Select the endpoint on the shared LAN.
On one PC, launch the executable twice and join `127.0.0.1:47620`.

Native launch automation uses the same room handlers as the lobby.
`--lan-host-config <path>` reads a JSON object with `room`,
`host_country_id`, `guest_country_id`, `ai_preset`, `grace_minutes`
and `port`. Append `--lan-autostart` to start its clock.
`--lan-join <IP:port> <player-name>` joins a running room.

The guest's LAN panel shows verified checksum and mismatch counts. The
guest receives authoritative changes and renders them. The host provides
the AI decision log.

A broken connection retries automatically with the same private resume
token. The host keeps the guest's human country held during the configured
grace period. After that period, the host may explicitly assign temporary
AI. Reconnection restores the guest's human control and installs a fresh
snapshot. Resume tokens live in the running guest process. To replace a
guest who has permanently closed the app, recreate the room.

## LAN troubleshooting

- On the Windows Firewall prompt, allow the executable on the Private
  network profile. TCP 47620 carries game traffic. mDNS discovery uses
  UDP 5353. A custom room port changes the TCP port.
- If discovery is empty, enter the host's LAN address and room port
  manually. Run `ipconfig` on the host to find the adapter's IPv4 address.
  A VPN or virtual adapter can add other discovered endpoints.
- Both players need connectivity on the same LAN. Router guest networks,
  AP isolation and client isolation can prevent devices from reaching
  each other. Check the router's network configuration or use a reachable
  Tailscale or ZeroTier address with manual joining.
- A handshake rejection identifies a protocol or map-version mismatch.
  Use the same build and map on both instances.
- If the guest shows Reconnecting, keep the host room open. The guest
  blocks commands until its fresh snapshot is installed.

## Scope and measurements

The milestone index is `docs/ROADMAP.md`. Save and map versions are
documented in `docs/DATA_FORMATS.md`, network behavior in `docs/NETWORK.md`,
and machine measurements in `docs/DECISIONS.md`.

The owner has one Windows PC. M5 is exercised with separate native host
and guest processes on that PC. Cross-PC compatibility is unmeasured.
