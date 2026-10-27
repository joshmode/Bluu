# Diagnostic export schema (v1)

```json
{
  "schema": "bluu.diagnostics.v1",
  "generated_at": "2026-10-27T00:00:00Z",
  "application": "Bluu 0.1.0",
  "notice": "Local export ...",
  "privacy": {
    "redact_device_names": true,
    "redact_addresses": true,
    "redact_endpoint_ids": true,
    "redact_paths": true
  },
  "snapshot": {
    "generated_at": "...",
    "system": { "os_name": "Windows 11", "os_version": "...", "os_build": 26100 },
    "backend": { "id": "windows-native", "name": "...", "read_only": true },
    "adapter": { "name": "Bluetooth adapter", "classic_supported": true },
    "devices": [
      {
        "name": "Bluetooth device 1",
        "connected": true,
        "profiles": [
          { "profile": "a2dp_sink", "version": "1.4", "evidence": "observed" }
        ],
        "association": { "method": "container_id", "confidence": "high" }
      }
    ],
    "endpoints": [
      {
        "friendly_name": "Audio endpoint 1 · output · active",
        "flow": "render",
        "state": "active",
        "shared_mode_format": { "sample_rate_hz": 48000, "channels": 2 }
      }
    ],
    "active_codec": null,
    "warnings": [],
    "notes": []
  }
}
```

The schema is versioned. Additive fields require no version bump; changing meaning or
removing fields requires `bluu.diagnostics.v2`. Tests should assert that default
exports do not contain known identifiers from a fixture snapshot.
