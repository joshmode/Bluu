# Testing and validation

## What has been run on this host

Host: macOS (Apple Silicon), no Windows Bluetooth radio.

Run:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --target x86_64-pc-windows-msvc
cargo run -p bluu-platform --bin screenshots
cmake -S native/owb -B build/native-host
cmake --build build/native-host --target owb_driver_tests
build/native-host/tests/driver/owb_driver_tests
```

Results:

- Core tests: 31 passed (codec IDs, provider contracts, evidence rules, SDP
  bounds/fixtures, feasibility, privacy redaction, settings, verification).
- Platform tests: 9 passed (screenshot invariants, helper argument/path validation,
  provider discovery/traversal rejection, demo provider regression).
- App tests: 3 passed.
- Provider-contract integration tests: 4 passed (apply/verify/rollback round-trip,
  missing-executable rejection, malformed-executable failure, contained
  install/uninstall).
- Native provider adapter tests: 6 passed.
- Native AVDTP/packet/receive-queue host tests: 50 passed (SEP discovery, codec
  capability matching, SET_CONFIGURATION bytes, LDAC frequency codes, per-codec
  RTP/payload framing, transactions, fragmentation, abort, timeout/backoff, and
  bounded receive queueing).
- Native aptX core: compiled and executed on the host with an independent
  block-level smoke harness (not a substitute for Windows service tests).
- Platform screenshot test: passed.
- Windows target: type-checked successfully (not linked or executed; MSVC runtime and
  Windows host are required for a real build).
- UI/demo: source-level check only on this host; no real Windows window was launched.

## Required hardware validation (not yet done)

Fill this matrix with concrete evidence. Do not mark “verified” based on the app's own
label.

| ID | PC / Windows build | Radio + driver | Headset/speaker | Profiles | Advertised codecs | Active codec observed | Independent verification | Result |
|---|---|---|---|---|---|---|---|---|
| HW-01 | Windows 11 24H2, Intel | Intel AX211 | SBC/AAC headset | A2DP, HFP | TBD | TBD | ETW + playback | Pending |
| HW-02 | Windows 11 23H2, AMD | Realtek | aptX headset | A2DP, HFP | TBD | TBD | ETW + playback | Pending |
| HW-03 | Windows 10 22H2 | Vendor adapter | SBC-only | A2DP | TBD | TBD | ETW + playback | Pending |
| HW-04 | Windows 11 24H2 | Qualcomm radio | aptX Adaptive headphones | A2DP, HFP | TBD | TBD | Vendor + ETW | Pending |
| HW-05 | Windows 11 24H2 | LE Audio-capable | LE Audio earbuds | LE Audio | PACS/BAP | Not via A2DP | Driver tools | Pending |
| HW-06 | Windows 11 24H2 | Sony-compatible dongle/stack | LDAC headphones | A2DP | TBD | TBD | External analyzer | Pending |

Use `scripts/windows/hardware-validation.ps1` to collect the evidence bundle for each
row. Use `scripts/windows/smoke-provider.ps1` for the no-headset provider checks.
The bundle is local; never mark a row verified from Bluu's selected-codec label.

## Real-device checklist

1. Pair and connect the device in Windows Settings.
2. Confirm it appears in Bluu with correct connection state.
3. Confirm cached A2DP/HFP profile observations are plausible; do not infer codec
   support from them.
4. Confirm endpoint association method and confidence.
5. Play stereo audio; run “Verify live codec” and record the codec IDs/timestamp.
6. Repeat with the headset microphone open; document HFP switching and endpoint
   change.
7. Disconnect/reconnect; verify selected device handling and stale-info behaviour.
8. Export diagnostics and verify redaction.
9. Capture a WPR/ETL trace for independent comparison; do not share traces with
   sensitive device data.

## Performance measurement

Measure on real Windows hardware:

- cold start to first interactive frame;
- scan wall time and endpoint count;
- idle CPU over 60 s with no audio;
- working set after first scan;
- capture helper peak CPU during 30 s.

`scripts/windows/hardware-validation.ps1` records a demo-mode cold-start proxy,
working set, and idle CPU sample under `performance.json`. These are not native
service or Bluetooth transmission benchmarks; repeat the measurement with the
native service and a real headset before making release claims.

## Failure injection

- No Bluetooth radio.
- Bluetooth off.
- Device disappears during scan.
- Empty SDP cache.
- Endpoint exists but is disabled.
- UAC cancelled.
- Helper fails to start.
- Trace session name already in use.
- Unparseable helper result.
- Settings file corrupt.
- Export directory unwritable.

The app should report a specific error and preserve prior state.
