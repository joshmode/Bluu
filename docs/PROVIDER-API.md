# Bluu advanced codec provider API v1

This is the executable contract for providers that can offer LDAC, aptX HD,
aptX Adaptive/Lossless, or another codec not available in the Windows in-box A2DP
stack. A provider owns the codec encoder and Bluetooth/audio transport; Bluu owns
validation, preference ordering, invocation, rollback, and evidence labeling.

## Installation layout

```text
%LOCALAPPDATA%\Bluu\providers\<provider-id>\
  provider.json
  provider.exe
```

or beside the app:

```text
<bluu directory>\providers\<provider-id>\
```

`BLUU_PROVIDER_DIR` is a development/testing override. The executable must resolve
inside the provider directory. Absolute paths and `..` are rejected.

## Manifest

See `docs/ADVANCED-CODEC-PROVIDER-PLAN.md` for the full shape. Example:

```json
{
  "api_version": 1,
  "id": "vendor.ldac",
  "name": "Vendor LDAC provider",
  "version": "1.2.0",
  "executable": "provider.exe",
  "publisher": "Vendor",
  "requires_elevation": true,
  "stack": "replacement_a2dp_driver",
  "capabilities": {
    "preferred_codec": true,
    "priority": true,
    "parameters": true,
    "active_codec": true,
    "rollback": true,
    "dry_run": true
  },
  "codecs": [
    {
      "codec": "ldac",
      "parameters": [
        {
          "kind": "choice", "key": "quality", "label": "LDAC quality",
          "options": ["330", "660", "990"], "default": "990"
        }
      ]
    },
    {"codec": "aptx_hd"},
    {"codec": "aptx_adaptive"},
    {"codec": "sbc"}
  ]
}
```

## Invocation

```text
provider.exe --request <absolute-request.json> --result <absolute-result.json>
```

Bluu creates both files in a private temporary directory and enforces a 120-second
timeout and a 128 KiB result limit. The provider must exit 0 and write a valid
response. Elevated providers are launched with `runas`.

## Actions

| Action | Requirements | Response |
|---|---|---|
| `describe` | device optional | capabilities/health |
| `apply` | connected device + validated preference | accepted, reconnect flag, rollback token optional |
| `verify` | connected device | active codec + verification method |
| `rollback` | token optional | acknowledgement |

`ok: true` only means the provider accepted the request. It is not proof of codec
negotiation. `active_codec` is shown as provider-reported and remains unverified
unless the provider supplies a device-bound verification method and Bluu's evidence
rules accept it.

## Validation rules

- API version must be 1.
- Provider ID: letters/digits/`.`/`-`/`_`, max 96.
- Executable must be relative and contained.
- No duplicate codecs. Parameter keys must be globally unique across codecs because
  the preference map is flat; use codec-qualified keys such as `ldac_frequency`.
- Priority requires at least two codecs.
- Parameter defaults must be valid and values must be validated on apply.
- Unknown codecs are rejected.

## Provider responsibilities

- Obtain all codec, patent, trademark, and redistribution rights.
- Ship/install/sign the driver or service according to Windows requirements.
- Validate local radio, remote device capability, multipoint state, and reconnect.
- Preserve prior state and support rollback.
- Never claim active transmission without evidence.
- Avoid logging Bluetooth addresses or audio content.

## Demos and tests

`bluu-provider-demo` is a simulated provider. It never touches Bluetooth or audio.
Build it, copy its manifest/executable into a provider directory, then run:

```bash
BLUU_PROVIDER_DIR=<dir> cargo run -p bluu-platform --bin providers-smoke
```

For a no-headset package check that does not require elevation, use:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/windows/smoke-provider.ps1
```

It verifies manifest identity, executable containment, SHA-256 consistency, and the
`--manifest` output emitted by the provider executable. It does not contact the
native service or claim that a codec is active.

## Native provider

`providers/bluu-native-a2dp` is the optional in-repo adapter for the experimental
native driver/service. Its parameter keys are `ldac_quality`, `ldac_frequency`,
`aptx_hd_frequency`, and `aptx_frequency`. It reports driver state as
provider-attested, never as independent verification.

## Installation in the UI

The Providers page has **Install provider folder…**. It validates the manifest,
requires the executable to be contained, copies the complete package into
`%LOCALAPPDATA%\Bluu\providers\<id>`, re-discovers it, and removes the copy if
validation fails. **Remove provider package** only removes packages inside that
managed directory. Bluu still does not install a vendor driver/service; the
publisher's own signed installer must do that where required.
