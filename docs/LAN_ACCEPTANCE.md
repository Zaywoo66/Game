# M5 LAN acceptance

Use the same Windows build on two physical PCs. Download
`strategy-windows-x64` from a successful GitHub Actions run for the commit
under test. Extract a copy on each PC. Record the commit and CI run URL.

## Before starting

- Connect both PCs to the same non-guest network. Check that the router or
  access point permits traffic between clients.
- If Windows Firewall asks, allow the game on the private network. Record
  whether a prompt appeared. The host listens on TCP 47620 by default and
  room discovery uses mDNS on UDP 5353.
- On the second PC, run the WebGL smoke page and copy the renderer string.
  Run `strategy-app.exe --map-benchmark` and record the resolution, mean FPS
  and p95 frame time. Record the results in D4 with the build and GPU.
- Launch the two game instances with separate, unused log paths. Run these
  commands from the directory containing the executable. Each command
  prints and saves the actual process exit code after the window closes.

```powershell
# Host PC
$game = Start-Process .\strategy-app.exe -ArgumentList '--lan-log', 'host-lan.log' -PassThru -Wait -WindowStyle Normal
"exit_code=$($game.ExitCode)" | Tee-Object -FilePath host-exit.txt
```

```powershell
# Guest PC
$game = Start-Process .\strategy-app.exe -ArgumentList '--lan-log', 'guest-lan.log' -PassThru -Wait -WindowStyle Normal
"exit_code=$($game.ExitCode)" | Tee-Object -FilePath guest-exit.txt
```

## Session checklist

1. Create a room on the host. Search for the room on the guest before
   entering an address. Record whether it appeared via mDNS and whether
   joining via discovery worked. If it failed, try the host's LAN IP and
   port and record the manual connection result separately.
2. Play for at least 60 minutes of wall-clock time. Use speeds 1–3,
   pause and resume, and have the host change speed at least once.
   Exercise commands from both players.
3. Disconnect the guest from the network once while leaving the game
   process open. Restore the connection and check that the guest
   reconnects, sees current state and retains control of its country.
   Note the disconnect and recovery times.
4. Close both windows normally and collect both LAN log files and both
   exit-code files. Attach them to the acceptance report.

The per-minute records contain the latest checksum tick and SHA-256,
cumulative checksum and resync counts, and the sample count and
minimum, mean and maximum ping round-trip times. Times are Unix
milliseconds. Compare checksums only when their ticks match. The host
counts frames sent, and the guest counts verified checksums and detected
mismatches. Recovery snapshots count as resyncs. Initial and reconnect
handshake snapshots do not. A missing checksum or ping sample is
reported as `none`.

A normal app exit writes an `app_exit` footer. Leaving a room or exiting
the app prints the session summary and writes it to the log. If an app
exits unexpectedly, preserve its final log and external process exit
code, even when the footer is absent.

## Pass criteria

At least 60 minutes on two physical PCs, with the requested controls
and reconnect exercised, zero unexplained checksum mismatches and no
crash. Record the discovery and firewall results even if manual IP works.
A same-PC accelerated run is supporting evidence for the simulation and
protocol. D4 and M5 remain open until the physical-PC checks are recorded.


## Existing local evidence

The original same-PC Tauri loopback run logged 37 verified checksums and
zero mismatches across 18 minutes 41 seconds. Both processes exited before
the hour. The monitor only polled process IDs and did not capture exit codes
or issue a stop. The available application and Windows logs contain no
record that identifies a crash, window closure or external termination. The
cause is undetermined.

The updated native app was launched as two Tauri processes with distinct
WebView2 user-data folders. Both joined the same loopback room and recorded
RTT samples. Closing each main window returned process exit code 0 and wrote
`event=session_summary reason=app_exit` plus `event=app_exit exit_code=0` to
both logs. This 15-second shutdown check confirms the current build flushes
the report on normal window close. It does not identify why the earlier
18-minute run ended or replace the one-hour two-PC acceptance.

The headless seed-1440 simulation advanced from 1440-01-01 to 1790-01-01
(127,836 daily ticks) in 12.3 seconds on the available PC and exited zero.
It ended with six countries and one territorial treaty. This run checks
simulation invariants. The accelerated protocol v2 TCP run on one Windows PC advanced the same
127,836 days in 354.5 seconds. The guest verified all 4,261 checksums
with zero mismatches and zero recovery snapshots. Exact save bytes
matched the host and an independent simulation at every checkpoint and
at the final tick. To repeat it:

```powershell
cargo test --locked -p strategy-app --test lan_replay lan_guest_matches_host_from_1440_through_1790 -- --ignored --exact --nocapture
```
