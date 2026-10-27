# Security model and threat analysis

## Non-negotiable product rules

- No arbitrary registry edits.
- No undocumented registry modifications.
- No disabling Secure Boot or driver signature enforcement.
- No unsigned or silently installed drivers.
- No system-file replacement.
- No automatic service termination or Bluetooth stack replacement.
- No hidden background service.
- No network access in the application.

## Privilege model

The main application is a standard-user process. It has no requestedExecutionLevel
`requireAdministrator` manifest. Discovering devices and reading endpoints/formats
does not require elevation in the normal desktop case.

The only privileged component is `bluu-etw-helper.exe`:

| Item | Design |
|---|---|
| Trigger | Explicit “Verify live codec” action |
| Elevation | `ShellExecuteExW` with `runas`; cancellation is reported |
| Lifetime | One bounded trace session; exits |
| Input | `--etw-helper`, seconds (5–120), result path |
| Output | Minimal JSON result file with codec IDs |
| Writes | Result file only |
| Registry/driver/service | None |
| Network | None |

## Threat model

| Threat | Impact | Mitigation |
|---|---|---|
| Malicious device sends hostile SDP | Parser recursion/overflow | Bounds checks, max depth 16, no panics on truncation; tests |
| Device name spoofing | Misassociation | Name fallback is Low confidence and labelled; container ID preferred |
| Fake app files/helper | Elevation of untrusted binary | Code signing/installer SHA-256; helper must sit next to app; verify release artifacts |
| Result file parsing | Memory/logic abuse | Small serde struct; size limits recommended |
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

Bluu v0.1.0 makes no system configuration changes, so there is no driver or registry
rollback to design. If a future privileged backend is proposed:

1. It must ship a signed rollback/uninstall path.
2. It must support a dry run and explicit consent.
3. It must preserve the original driver/state.
4. It must document recovery without the GUI.
5. It must pass a separate security review and hardware matrix.

The ETW helper leaves no persistent system state. If it is killed, stop the
`Bluu-A2DP-Verify` session with standard ETW tools; the app reports the failure.
