# Privacy

Bluu is local-first.

- No account or sign-in.
- No telemetry.
- No cloud service.
- No network code in the app or helper.
- No automatic diagnostic upload.
- No collection of Bluetooth addresses for analytics.

## What stays on the machine

Settings and logs live under the platform application-data directory. Diagnostic
exports are written only when explicitly requested.

## Diagnostic exports

Default export redacts:

- Bluetooth addresses;
- device names;
- endpoint and container IDs;
- local filesystem paths.

Included: Windows version/build, backend capabilities, connection/profile
observations, endpoint formats, codec inventory, and findings.

Not included: usernames, computer name, audio samples, packet captures, credentials,
browsing data.

Redaction is a safety default, not an absolute guarantee. Review exports before
sharing. Users can explicitly opt into including device names or addresses from
Settings; those choices are recorded in the export metadata.
