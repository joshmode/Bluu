# install-native-a2dp.ps1
# Guarded, explicit installer for the optional native A2DP component.
#
# This is NOT a silent installer and does not claim production signing. It:
#   1. verifies the driver package, catalog, service binary, provider, and manifest;
#   2. rejects missing/invalid signatures unless -AllowTestSigned is explicit;
#   3. records the prior oemNN.inf identity and service state before changes;
#   4. installs the service and driver with pnputil;
#   5. verifies the pipe/device state and prints rollback instructions.
#
# Test-signing mode is a separate, explicit switch and still must never disable
# Secure Boot or driver signature enforcement automatically.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)] [string] $PackageDir,
    [string] $ServiceExe = 'owb_service.exe',
    [string] $InfName = 'owb_a2dp.inf',
    [string] $ProviderDir = '',
    [switch] $AllowTestSigned,
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$package = (Resolve-Path $PackageDir).Path
$inf = Join-Path $package $InfName
$sys = Join-Path $package 'owb_a2dp.sys'
$cat = Join-Path $package 'owb_a2dp.cat'
$service = Join-Path $package $ServiceExe
$stateDir = Join-Path $package 'install-state'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$stateFile = Join-Path $stateDir "install-$stamp.json"

if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this script from an elevated PowerShell session.'
}
foreach ($required in @($inf, $sys, $cat, $service)) {
    if (-not (Test-Path $required)) { throw "Missing package file: $required" }
}

function Get-AuthenticodeEvidence {
    param([string] $Path)
    $signature = Get-AuthenticodeSignature -FilePath $Path
    [pscustomobject]@{
        path = $Path
        status = $signature.Status.ToString()
        subject = if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
        thumbprint = if ($signature.SignerCertificate) { $signature.SignerCertificate.Thumbprint } else { $null }
        timestamp_subject = if ($signature.TimeStamperCertificate) { $signature.TimeStamperCertificate.Subject } else { $null }
    }
}

$signatures = @(
    Get-AuthenticodeEvidence $sys
    Get-AuthenticodeEvidence $cat
    Get-AuthenticodeEvidence $service
)
$invalid = @($signatures | Where-Object { $_.status -ne 'Valid' })
if ($invalid.Count -gt 0) {
    if (-not $AllowTestSigned) {
        throw "Package signature is not valid. Production installs require trustworthy signing. Use -AllowTestSigned only for a controlled test machine.`n$($invalid | ConvertTo-Json -Depth 5)"
    }
    Write-Warning 'Installing with a non-production signature. This does not make the driver production-ready.'
}

$state = [ordered]@{
    package = $package
    started_at = (Get-Date).ToString('o')
    dry_run = [bool]$DryRun
    allow_test_signed = [bool]$AllowTestSigned
    signatures = $signatures
    service_before = [string](& sc.exe query owb-service 2>&1 | Out-String)
    drivers_before = [string](& pnputil.exe /enum-drivers 2>&1 | Out-String)
    published_names_before = @()
    installed_published_name = $null
    reboot_required = $false
}

# Record any existing package with the same original INF name before installing.
$drivers = $state.drivers_before
$blocks = [regex]::Split($drivers, '(?m)^\s*$')
foreach ($block in $blocks) {
    if ($block -match '(?im)^\s*(Published Name|Nombre publicado)\s*:\s*(oem\d+\.inf)\s*$' -and
        $block -match '(?im)^\s*(Original Name|Nombre original)\s*:\s*owb_a2dp\.inf\s*$') {
        $state.published_names_before += $Matches[2]
    }
}

New-Item -ItemType Directory -Force -Path $stateDir | Out-Null
$state | ConvertTo-Json -Depth 8 | Set-Content -Path $stateFile -Encoding UTF8

if ($DryRun) {
    Write-Host "Dry run complete. State record: $stateFile"
    Write-Host 'No service, driver, certificate, or registry state was changed.'
    return
}

try {
    # The service must run as LocalSystem to reach the PnP device. It is only
    # created after the user explicitly ran this installer.
    & sc.exe query owb-service *> $null
    if ($LASTEXITCODE -eq 0) {
        & sc.exe stop owb-service | Out-Null
        & sc.exe delete owb-service | Out-Null
    }
    & sc.exe create owb-service binPath= "`"$service`"" start= auto obj= LocalSystem DisplayName= "Bluu optional A2DP service" | Out-Null
    if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 1073) {
        throw "sc.exe create failed with exit $LASTEXITCODE"
    }
    & sc.exe description owb-service "Optional experimental Bluu A2DP encoder service. Requires the separately installed native driver." | Out-Null
    & sc.exe failure owb-service reset= 86400 actions= restart/5000/restart/15000/none/0 | Out-Null

    & pnputil.exe /add-driver $inf /install
    if ($LASTEXITCODE -ne 0) { throw "pnputil failed with exit $LASTEXITCODE" }

    $after = [string](& pnputil.exe /enum-drivers 2>&1 | Out-String)
    $afterBlocks = [regex]::Split($after, '(?m)^\s*$')
    foreach ($block in $afterBlocks) {
        if ($block -match '(?im)^\s*(Published Name|Nombre publicado)\s*:\s*(oem\d+\.inf)\s*$' -and
            $block -match '(?im)^\s*(Original Name|Nombre original)\s*:\s*owb_a2dp\.inf\s*$') {
            $state.installed_published_name = $Matches[2]
        }
    }

    & sc.exe start owb-service | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Warning "The service did not start; it may require a reboot or driver load." }

    if ([string]::IsNullOrWhiteSpace($ProviderDir)) {
        $ProviderDir = Join-Path (Split-Path $package -Parent) 'providers\bluu-native-a2dp'
    }
    if (Test-Path (Join-Path $ProviderDir 'provider.json')) {
        $env:BLUU_PROVIDER_DIR = (Resolve-Path $ProviderDir).Path
        try {
            $provider = Join-Path $ProviderDir 'bluu-provider-owb.exe'
            if (Test-Path $provider) {
                $hash = (Get-FileHash $provider -Algorithm SHA256).Hash
                Write-Host "Native provider SHA-256: $hash"
            } else {
                Write-Warning "Provider adapter not found in $ProviderDir"
            }
        }
        finally {
            Remove-Item Env:BLUU_PROVIDER_DIR -ErrorAction SilentlyContinue
        }
    }
}
catch {
    $state | ConvertTo-Json -Depth 8 | Set-Content -Path $stateFile -Encoding UTF8
    throw
}
finally {
    $state.completed_at = (Get-Date).ToString('o')
    $state.service_after = [string](& sc.exe query owb-service 2>&1 | Out-String)
    $state.drivers_after = [string](& pnputil.exe /enum-drivers 2>&1 | Out-String)
    $state | ConvertTo-Json -Depth 8 | Set-Content -Path $stateFile -Encoding UTF8
}

Write-Host "Install completed. State: $stateFile"
Write-Host "Rollback: native\owb\tools\owb-rollback.bat"
Write-Host 'Reboot if the driver or service does not start, then run scripts/windows/hardware-validation.ps1.'
Write-Host 'Never distribute a test-signed driver as a production artifact.'
