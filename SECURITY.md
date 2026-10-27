# Security policy

## Supported versions

The current release line is supported for security reports. Pre-release builds may
change without notice.

## Reporting a vulnerability

Do not open a public issue for a security vulnerability. Contact the maintainer
privately through the repository's security advisory feature or the contact listed
in `Cargo.toml`.

Include:

- affected version;
- Windows build and architecture;
- reproduction steps;
- impact assessment;
- whether elevation/ETW/driver code is involved.

Please avoid including real Bluetooth addresses, packet captures, or personal data in
reports. Redact logs.

## Scope

In scope: privilege boundaries, the ETW helper/provider argument and path handling, parser
panics/memory safety, export redaction, supply-chain/licence issues, and unsafe
Windows FFI usage.

Out of scope: third-party provider driver defects, codec licensing disputes, and
issues requiring a malicious kernel driver or already-compromised administrator
account. A provider must publish its own signing, recovery, and support policy.
