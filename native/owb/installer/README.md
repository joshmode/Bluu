# Native A2DP installer status

**The MSI project in this directory is quarantined and must not be built or
distributed.** `Product.wxs` and `OpenWinBlue.wixproj` are stale upstream files
that reference a missing GUI, install a test certificate into Root and
TrustedPublisher, ignore custom-action failures, and can install a test-signed
driver without a safe rollback record.

Use the reviewed PowerShell flow instead:

- `../tools/install-native-a2dp.ps1` performs explicit, guarded install steps.
- `../tools/owb-rollback.bat` remains the emergency recovery entry point.
- `docs/NATIVE-A2DP.md` documents prerequisites and release gates.

Production distribution still requires a Microsoft-attestation-signed driver and
a hardware-validated package. The scripts never disable Secure Boot or driver
signature enforcement.
