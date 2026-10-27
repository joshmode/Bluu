# Windows compatibility matrix

Status: code targets documented APIs; hardware validation pending.

| Capability | Windows 10 2004+ | Windows 11 21H2+ | Windows 11 22H2+ | Windows 11 24H2+ | Notes |
|---|---|---|---|---|---|
| Paired device enumeration | Expected | Expected | Expected | Expected | WinRT; hardware unverified |
| Connection state | Expected | Expected | Expected | Expected | WinRT |
| Cached SDP profile read | Expected | Expected | Expected | Expected | Cache can be empty |
| Core Audio endpoints/formats | Expected | Expected | Expected | Expected | Documented |
| ETW active-codec observation | Expected | Expected | Expected | Expected | Undocumented payload; needs admin |
| AAC in-box A2DP | Not listed | Listed | Listed | Listed | Microsoft table |
| aptX Classic in-box | Listed 1507+ | Listed | Listed | Listed | Radio/driver caveats |
| aptX Adaptive | No | No | No | Select Qualcomm devices | OEM-specific |
| LE Audio | No | 22H2+ vendor path | Vendor path | Vendor path | ACX/VSAP |
| Shared-mode codec preference | No supported in-box API | No supported in-box API | No supported in-box API | No supported in-box API | Provider-gated advanced path |
| LDAC / aptX HD via native provider | Experimental | Experimental | Experimental | Experimental | Requires capable remote, test-signed/attested driver, and hardware validation |
| BthA2dp AAC toggle | Not offered | Build-sensitive | Build-sensitive | Build-sensitive | Undocumented; backup, consent, reconnect |

## Architecture support

| Architecture | Build status | Hardware status |
|---|---|---|
| x86_64 Windows | Cross-compile checked | Unverified (no Windows host) |
| ARM64 Windows | Not yet release-verified | Unverified |
| macOS (development/demo) | Builds and runs demo | N/A |

## Driver/OEM caveats

- A Bluetooth radio may support Classic, LE, or both.
- Some OEM drivers add codec paths not present on a generic Microsoft stack.
- Endpoint IDs, container IDs, and SDP cache behavior vary by driver and Windows build.
- The application should be revalidated after major Windows feature updates.
