# Test fixtures

This directory is reserved for:

- sanitized SDP byte records;
- redacted helper JSON results;
- endpoint/WAVEFORMATEX fixtures;
- diagnostic export examples with all identifiers replaced.

Rules:

- Never commit real Bluetooth addresses, device names, usernames, or ETL files.
- A fixture must include the source/format and the expected evidence classification.
- Parsing fixtures are property-tested against truncation/malformed input.
