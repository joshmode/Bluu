# Security model and threat analysis

## Non-negotiable product rules

- No arbitrary registry edits.
- One narrow, explicitly requested BthA2dp AAC value is supported. It is not a
  general registry editor.
- No disabling Secure Boot or driver signature enforcement.
- No silently installed drivers.
- No system-file replacement.
- No automatic service termination or Bluetooth stack replacement.
- No hidden background service.
- No network access in the application.

## Privilege model

The main application is a standard-user process. It has no requestedExecutionLevel
`requireAdministrator` manifest. Discovering devices and reading endpoints/formats
does not require elevation in the normal desktop case.

Bluu ships one small elevated helper with two fixed operations: bounded ETW
observation, and the backed-up BthA2dp AAC preference. An advanced provider can be
marked `requires_elevation`; it is launched only after explicit consent:

| Item | Design |
|---|---|
| Trigger | Explicit “Verify live codec” action |
| Elevation | `ShellExecuteExW` with `runas`; cancellation is reported |
| Lifetime | One bounded trace session; exits |
| Input | Fixed ETW arguments, or `--aac-helper --set-aac 0/1|--restore-aac` |
| Output | Minimal JSON result file |
| Writes | Result file; AAC mode writes one BthA2dp DWORD plus backup values |
| Driver/service | None |
| Network | None |

The portable app does not install a driver or service. The optional in-repo native
provider has its own installer and test-signing procedure; it is never started
silently. Production distribution requires a properly signed driver and must not
ask users to disable Secure Boot.

## Threat model

| Threat | Impact | Mitigation |
|---|---|---|
| Malicious device sends hostile SDP | Parser recursion/overflow | Bounds checks, max depth 16, no panics on truncation; tests |
| Device name spoofing | Misassociation | Name fallback is Low confidence and labelled; container ID preferred |
| Fake app files/helper | Elevation of untrusted binary | Code signing/installer SHA-256; helper must sit next to app; verify release artifacts |
| Elevated result-file race | Overwrite/symlink abuse | Helper creates its JSON result with `create_new`; existing paths fail closed |
| Result file parsing | Memory/logic abuse | Small serde struct with 64 KiB ETW / 16 KiB AAC limits |
| Trace provider spoofing | False codec observation | Provider GUID fixed; evidence is unverified without independent trace |
| Diagnostics leak identifiers | Privacy | Redacted by default; explicit opt-in |
| Log injection | Log confusion | Structured tracing, no terminal control sequence execution |
| UAC prompt wrapping | User confusion | Explicit disclosure in UI and docs; no auto-elevation |
| Path traversal in export | Write outside chosen folder | App-generated filename/timestamp under app data |

## Sensitive-data handling

Diagnostic exports include OS build, backend capabilities, endpoint formats,
profiles, codec inventory, and findings. By default they redact Bluetooth addresses,
device names, endpoint/container IDs, and paths. Usernames, computer name, audio
samples, packet captures, and credentials are never collected.

It is not possible to make redaction a legal guarantee. Users must review exports.

## Rollback and recovery

The AAC toggle saves the prior value and exposes **Restore previous value**.
It affects the whole PC and requires reconnect. The native provider's emergency
rollback script and exporter are documented in [NATIVE-A2DP.md](NATIVE-A2DP.md);
recovery must be available without the GUI before installing a driver.

The ETW helper leaves no persistent trace session after a normal exit. If it is
killed, stop the `Bluu-A2DP-Verify` session with standard ETW tools; the app
reports the failure.
