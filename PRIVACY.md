# Privacy Policy

**Effective date:** 28 October 2026

Bluu is a local-first Windows Bluetooth audio diagnostics application. This policy
explains what information Bluu processes, where it is stored, and the choices you
have. It applies to the Bluu desktop application, the ETW helper, and the optional
native audio provider.

## Summary

- Bluu does not require an account.
- Bluu does not include telemetry, analytics, advertising, or automatic uploads.
- Bluu does not sell or share personal information.
- Diagnostic exports are created only when you request one, and are saved locally.
- Bluu is designed to work without a cloud service or network connection.

## Information Bluu processes

Bluu reads information from Windows that is needed to diagnose Bluetooth audio:

- paired and connected Bluetooth audio devices;
- Bluetooth connection state and cached A2DP/HFP profile information;
- Windows audio endpoints, audio formats, and timing details;
- locally installed codec capabilities and negotiated codec observations;
- the Windows version, build, and relevant backend capabilities;
- application settings and local diagnostic logs.

When you explicitly run **Verify live codec**, Bluu may start a short-lived elevated
ETW helper to observe Windows Bluetooth audio events. This is used to report the
codec Windows negotiated; it does not add telemetry or upload observations.

## Information Bluu does not collect

Bluu does not collect or transmit:

- usernames, computer names, or account credentials;
- browsing history, advertising identifiers, or usage analytics;
- audio recordings, audio content, or packet captures;
- Bluetooth addresses or device names for analytics;
- diagnostic reports automatically or in the background.

## Local storage and retention

Settings and logs are stored in the platform application-data directory on your
computer. Diagnostic exports are written only to a location you choose when you
request an export. Bluu does not set an automatic retention period. Local data
remains until you remove it, for example by deleting an export or clearing the
application's data directory.

## Diagnostic exports and redaction

By default, diagnostic exports redact Bluetooth addresses, device names, endpoint
and container identifiers, usernames, computer names, and local filesystem paths.
An export may contain Windows version details, connection and profile
observations, audio endpoint formats, codec inventory, and diagnostic findings.

You can optionally include device names or Bluetooth addresses in an export from
Settings. That choice is recorded in the export metadata. Redaction reduces risk
but is not a guarantee; review an export before sharing it.

## Sharing

Bluu does not share your information with third parties. If you choose to share a
diagnostic export, attach it to your own report or message. You control the
recipient and the information included.

Third-party audio providers or drivers that you choose to install are separate
software components. Their privacy practices are governed by their own policies.

## Security

Bluu uses local storage and does not operate a server that receives application
data. No storage or redaction method is completely secure. Keep diagnostic exports
private when they contain sensitive system information.

To report a security issue, follow [SECURITY.md](SECURITY.md). Do not include real
Bluetooth addresses, packet captures, or other personal data in a public issue.

## Your choices

You can:

- decline optional elevation and live codec verification;
- choose whether to include device names or addresses in an export;
- review, move, or delete local settings, logs, and diagnostic exports;
- avoid installing the optional native provider.

## Changes to this policy

This policy may be updated when Bluu's data handling changes. The effective date at
the top of this file will be revised, and material changes will be described in
the project changelog.

## Contact

For privacy questions, contact the maintainers through the project repository at
<https://github.com/joshmode/Bluu>. Please do not put personal or sensitive system
information in a public issue.

The technical implementation notes for local data handling are in
[docs/PRIVACY.md](docs/PRIVACY.md) and
[docs/PRIVACY-EXPORT-SCHEMA.md](docs/PRIVACY-EXPORT-SCHEMA.md).
