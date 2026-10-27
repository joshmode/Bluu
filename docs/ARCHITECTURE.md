# Architecture decision record

## ADR-001: Native Windows integration with an optional isolated trace helper

**Status:** accepted for v0.1.0 (2026-10-27).

### Context

The product brief asks for codec enabling, but Windows exposes no supported A2DP
codec preference/registration API. Shipping a kernel replacement would violate the
project's safety posture and cannot be validated on the development host. The best
product is therefore one that is honest about capability and excellent at
inventory/diagnostics/verification.

### Decision

1. Rust workspace with four crates:
   - `bluu-core`: pure domain, codec matrix, facts/evidence, diagnostics, SDP,
     privacy, settings, state transitions.
   - `bluu-platform`: backend trait implementations (`WindowsBackend`, `DemoBackend`).
   - `bluu-app`: egui desktop UI and worker thread.
   - `bluu-etw-helper`: separate, short-lived, elevated ETW observer.
2. The UI runs at standard integrity. It never requests administrator for scanning.
3. A scan uses documented WinRT/Core Audio interfaces and performs no writes.
4. Active codec observation is a separate user-initiated action; it launches the
   helper via `ShellExecuteExW` with the `runas` verb.
5. The helper accepts only `--etw-helper --seconds N --result-file PATH`, starts one
   trace session, writes a minimal JSON result, and exits. It has no registry,
   driver, service, or arbitrary-command path.
6. Codec facts are typed by evidence. A supported inventory item cannot be rendered
   as active. An ETW observation without a device identity is `ActiveUnverified`.
7. In-box configuration is limited to the explicitly consented, backed-up BthA2dp
   AAC value. The native optional provider adds a separately installed replacement
   A2DP path behind the same capability/evidence model.

### Consequences

- Users on generic hardware cannot enable LDAC/aptX HD through Bluu. This is a
  documented limitation, not a hidden failure.
- The app remains useful: device state, A2DP/HFP profile evidence, endpoint formats,
  codec inventory, requested ETW observation, redacted exports, and actionable
  diagnostics.
- Advanced codec support can be added later through a new `AudioBackend`
  implementation only after legal, signing, safety, and recovery analysis.

## Module boundaries

```text
┌────────────────────────────────────────────┐
│ bluu-app (egui, settings, worker, export)  │
└───────────────┬────────────────────────────┘
                │ trait AudioBackend
┌───────────────▼────────────────────────────┐
│ bluu-platform                              │
│  DemoBackend        WindowsBackend         │
│  (fixtures)          ├─ WinRT Bluetooth     │
│                      ├─ Core Audio          │
│                      └─ ETW helper launcher │
└───────────────┬────────────────────────────┘
                │ domain types + evidence
┌───────────────▼────────────────────────────┐
│ bluu-core                                   │
│  model · codec · feasibility · diagnostics  │
│  sdp · privacy · settings · verification    │
└────────────────────────────────────────────┘

bluu-etw-helper  ── one trace session ──► JSON result file
```

## Backend contract

`AudioBackend` exposes:

- descriptor and capability flags;
- `scan()` returning a snapshot;
- `capture_active_codec()` returning an observation or an explicit error.

Capability flags include device/profile/endpoint reads, observation, parameter
observation, preferred-codec configuration, parameter configuration, and endpoint
format control. v0.1.0 advertises reads and observation only. A future backend must
not advertise an operation it cannot perform and verify.

## Data model and state machine

Codec states:

```text
Unknown
  ├─ Unsupported
  ├─ UnavailableOnBackend
  ├─ HardwareOnly
  ├─ SupportedAvailable
  └─ ActiveUnverified ── independent evidence ──► ActiveVerified
```

Rules:

- Inventory (`documented`) is never active.
- An A2DP ETW event is observed but remains unverified without device identity.
- A device-qualified user/external trace may promote to verified only through
  `Verifier`, which rejects empty device IDs, unsupported methods, and stale
  evidence.
- The selected codec in the UI is only a view selection; it is not a setting.
- A disconnect invalidates endpoint/format associations on the next scan; evidence
  timestamps remain visible.

## Error model

`BackendError` distinguishes unsupported operations, device access, audio access,
helper startup, elevation cancellation, cancellation, platform failure, and invalid
input. Each has an actionable explanation. Errors are surfaced in the UI and never
silently converted into “unsupported hardware.”

## Security model

- Standard-user UI; no admin manifest.
- Helper is a separate executable, invoked only by explicit action.
- Helper input is a small parsed argument set; no shell interpolation of arbitrary
  commands. The path is passed as a quoted parameter to ShellExecuteExW.
- Temp result path is app-generated under the user's temp directory.
- The UI itself does not install a driver/service. The only registry write is the
  narrow BthA2dp AAC preference through the helper. Driver/service installation is a
  separate explicit action under the native provider's installer.
- No telemetry or network code.
- Diagnostics are local JSON, redacted by default.

### Known helper risks and mitigations

The helper runs elevated. Risks include:

| Risk | Mitigation |
|---|---|
| Fake/renamed helper binary | Locate only `bluu-etw-helper.exe` next to the trusted app; installer integrity checks |
| Arbitrary trace session interference | Fixed provider GUID, fixed session name, bounded duration, stop on exit |
| Malicious result path | App-generated path only; release helper should validate the path is under the user's temp directory |
| Trace payload sensitive data | Bluu reads only codec IDs; no packet payload logging |
| UAC fatigue | One-time action, explicit copy, no background elevation |

## UI decision

egui/eframe 0.31 was selected over WinUI 3 because:

- it is Rust-native across the whole workspace and builds on developers' non-Windows
  machines for the core/demo path;
- it supports a custom, low-overhead dark-first design with predictable layout;
- WinUI 3 would add a C#/C++ binding layer and Windows-only build burden without
  granting any additional Bluetooth API;
- this app is a compact inspector, not a heavy shell-integrated product.

The trade-off is that egui is not literally WinUI/Mica. The visual language is
nevertheless restrained and native-feeling, with keyboard navigation, a settings
page, focusable controls, and high-contrast palettes. If deep Windows shell
integration becomes essential, the platform/core split allows a WinUI shell later.

## Performance design

- One backend worker thread, no polling loop outside active actions.
- Scans are event-driven; no 1 Hz device polling.
- Dynamic WebView/browser stack is not included.
- egui repaint is requested only while scanning/capturing.
- Release profile uses thin LTO and strips symbols.
- Measurements must be collected on real Windows hardware; see
  `docs/TESTING.md`.
