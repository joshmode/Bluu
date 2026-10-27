@echo off
setlocal EnableExtensions
echo This legacy batch installer is quarantined.
echo.
echo It previously installed a test certificate into Root/TrustedPublisher and
echo ignored driver-install failures. It does not run those actions now.
echo.
echo Use the guarded PowerShell installer from an elevated prompt:
echo   powershell -ExecutionPolicy Bypass -File "%~dp0install-native-a2dp.ps1" -PackageDir "%~dp0..\.."
echo.
echo Read installer\README.md and docs\NATIVE-A2DP.md first.
exit /b 2
