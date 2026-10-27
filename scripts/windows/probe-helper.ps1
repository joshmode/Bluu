# probe-helper.ps1
# Small, dependency-free JSON probes used by the Windows validation scripts.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('snapshot', 'provider-smoke', 'redaction', 'aac-state', 'first-scan')]
    [string] $Mode,
    [string] $BluuRoot = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($BluuRoot)) {
    $BluuRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
}

function Invoke-BluuJson {
    param([string[]] $Arguments)
    $exe = Join-Path $BluuRoot 'target\x86_64-pc-windows-msvc\release\bluu.exe'
    if (-not (Test-Path $exe)) { throw "bluu.exe not found at $exe" }
    $output = & $exe @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "bluu.exe $($Arguments -join ' ') failed: $($output -join [Environment]::NewLine)"
    }
    return ($output -join [Environment]::NewLine)
}

function Get-FirstScanMetric {
    $log = Join-Path $env:LOCALAPPDATA 'Bluu\logs\bluu.log'
    if (-not (Test-Path $log)) {
        throw "Bluu log not found: $log"
    }
    $match = Get-Content $log -Tail 3000 |
        Select-String -Pattern 'first backend scan completed' |
        Select-Object -Last 1
    if (-not $match) {
        throw 'No first-scan completion record found in Bluu log'
    }
    [pscustomobject]@{
        log = $log
        line = $match.Line
        note = 'Elapsed time is measured from app startup to the first completed backend scan.'
    } | ConvertTo-Json -Depth 4
}

switch ($Mode) {
    'snapshot' {
        $raw = Invoke-BluuJson @('--snapshot-json')
        $json = $raw | ConvertFrom-Json
        [pscustomobject]@{
            captured_at = (Get-Date).ToString('o')
            device_count = @($json.devices).Count
            endpoint_count = @($json.endpoints).Count
            active_codec = $json.active_codec.codec
            provider_ids = @($json.providers | ForEach-Object { $_.manifest.id })
            inbox_codec_supported = [bool]$json.inbox_codec.supported
            raw_snapshot = $json
        } | ConvertTo-Json -Depth 40
    }
    'provider-smoke' {
        $manifest = Get-ChildItem -Path $BluuRoot -Recurse -Filter provider.json -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '[\\/]providers[\\/]bluu-native-a2dp[\\/]provider.json$' } |
            Select-Object -First 1
        if (-not $manifest) { throw 'shipped native provider manifest not found' }
        $providerDir = Split-Path $manifest.FullName -Parent
        $old = $env:BLUU_PROVIDER_DIR
        try {
            $env:BLUU_PROVIDER_DIR = $providerDir
            $output = Invoke-BluuJson @('--provider-smoke')
        }
        finally {
            if ($null -eq $old) { Remove-Item Env:BLUU_PROVIDER_DIR -ErrorAction SilentlyContinue }
            else { $env:BLUU_PROVIDER_DIR = $old }
        }
        [pscustomobject]@{
            manifest_path = $manifest.FullName
            provider_dir = $providerDir
            output = $output
        } | ConvertTo-Json -Depth 8
    }
    'aac-state' {
        & (Join-Path $BluuRoot 'target\x86_64-pc-windows-msvc\release\bluu.exe') --aac-state
        if ($LASTEXITCODE -ne 0) { throw "aac-state failed with exit $LASTEXITCODE" }
    }
    'first-scan' {
        Get-FirstScanMetric
    }
    'redaction' {
        $exportDir = Join-Path $env:LOCALAPPDATA 'Bluu\exports'
        $latest = Get-ChildItem $exportDir -Filter 'bluu-diagnostics-*.json' -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if (-not $latest) { throw "no diagnostic export found in $exportDir" }
        $text = Get-Content $latest.FullName -Raw
        $json = $text | ConvertFrom-Json
        $checks = [ordered]@{
            schema = $json.schema
            has_address_like = [bool]($text -match '(?i)([0-9a-f]{2}:){5}[0-9a-f]{2}')
            has_home_path = [bool]($text -match '(?i)[A-Z]:\\Users\\')
            has_user_profile = [bool]($text -match '(?i)Users\\[^\\"]+')
            device_names_redacted = @($json.snapshot.devices | ForEach-Object {
                $_.name -match '^(Bluetooth device|Bluetooth audio device|Audio device) \d+$'
            }) -notcontains $false
        }
        [pscustomobject]@{ path = $latest.FullName; checks = $checks } | ConvertTo-Json -Depth 8
    }
}
