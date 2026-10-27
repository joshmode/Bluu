# hardware-validation.ps1
# Windows hardware, provider, recovery, and performance evidence collector.
#
# This script never installs a driver. It records what the machine reports and
# invokes Bluu's read-only/scan/smoke surfaces. Run it elevated for pnputil and
# ETW capture metadata; use -SkipTrace when elevation is unavailable.

[CmdletBinding()]
param(
    [string] $Output = '',
    [string] $BluuRoot = '',
    [switch] $SkipTrace,
    [switch] $SkipProvider,
    [int] $TraceSeconds = 20,
    [int] $IdleSeconds = 60
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ([string]::IsNullOrWhiteSpace($BluuRoot)) {
    $BluuRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
}
if ([string]::IsNullOrWhiteSpace($Output)) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $Output = Join-Path $BluuRoot "dist\hardware-validation-$stamp"
}
New-Item -ItemType Directory -Force -Path $Output | Out-Null

$logPath = Join-Path $Output 'validation.log'
function Write-Evidence {
    param([string] $Message)
    $line = "$(Get-Date -Format o) $Message"
    Write-Host $Message
    Add-Content -Path $logPath -Value $line
}

function Save-Command {
    param(
        [string] $Name,
        [string] $FilePath,
        [string[]] $Arguments = @()
    )
    $path = Join-Path $Output $Name
    try {
        $output = & $FilePath @Arguments 2>&1
        $code = $LASTEXITCODE
        @(
            "command: $FilePath $($Arguments -join ' ')",
            "exit: $code",
            '',
            ($output -join [Environment]::NewLine)
        ) | Set-Content -Path $path -Encoding UTF8
        Write-Evidence "saved $Name (exit $code)"
    }
    catch {
        @(
            "command: $FilePath $($Arguments -join ' ')",
            "error: $($_.Exception.Message)"
        ) | Set-Content -Path $path -Encoding UTF8
        Write-Evidence "failed $Name`: $($_.Exception.Message)"
    }
}

function Invoke-Probe {
    param([string] $Name, [string] $Mode)
    $path = Join-Path $Output $Name
    try {
        & powershell -NoProfile -ExecutionPolicy Bypass -File `
            (Join-Path $PSScriptRoot 'probe-helper.ps1') `
            -Mode $Mode -BluuRoot $BluuRoot |
            Set-Content -Path $path -Encoding UTF8
        Write-Evidence "probe $Mode -> $path"
    }
    catch {
        @{ error = $_.Exception.Message; mode = $Mode } |
            ConvertTo-Json | Set-Content -Path $path -Encoding UTF8
        Write-Evidence "probe $Mode failed: $($_.Exception.Message)"
    }
}

function Get-TraceMetadata {
    $path = Join-Path $Output 'trace-metadata.json'
    $trace = [ordered]@{
        saved_at = (Get-Date).ToString('o')
        provider = 'Microsoft-Windows-Bluetooth-BthA2dp'
        requested_seconds = $TraceSeconds
        status = 'not_collected'
        reason = ''
        independent = $false
    }
    if ($SkipTrace) {
        $trace.reason = 'disabled by -SkipTrace'
    }
    elseif (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        $trace.reason = 'requires an elevated PowerShell process'
    }
    else {
        $wpr = Join-Path $env:WINDIR 'System32\wpr.exe'
        if (-not (Test-Path $wpr)) {
            $trace.reason = 'wpr.exe is unavailable on this Windows edition'
        }
        else {
            try {
                & $wpr -start GeneralProfile -start Bluetooth -filemode 2>&1 | Out-Null
                Start-Sleep -Seconds ([Math]::Max(5, $TraceSeconds))
                $etl = Join-Path $Output 'bluu-bluetooth.etl'
                & $wpr -stop $etl 2>&1 | Out-Null
                $trace.status = 'collected'
                $trace.etl = $etl
                $trace.note = 'WPR ETL is local evidence. It is not uploaded and must be reviewed before sharing.'
            }
            catch {
                try { & $wpr -cancel 2>&1 | Out-Null } catch {}
                $trace.reason = $_.Exception.Message
            }
        }
    }
    $trace | ConvertTo-Json -Depth 6 | Set-Content -Path $path -Encoding UTF8
}

function Get-PerformanceEvidence {
    $path = Join-Path $Output 'performance.json'
    $exe = Join-Path $BluuRoot 'target\x86_64-pc-windows-msvc\release\bluu.exe'
    $result = [ordered]@{
        captured_at = (Get-Date).ToString('o')
        idle_seconds = $IdleSeconds
        cold_start_ms = $null
        working_set_mb = $null
        idle_cpu_percent = $null
        note = ''
    }
    if (-not (Test-Path $exe)) {
        $result.note = "bluu.exe not found at $exe"
    }
    else {
        try {
            $sw = [Diagnostics.Stopwatch]::StartNew()
            $process = Start-Process -FilePath $exe -ArgumentList '--demo' -PassThru
            Start-Sleep -Milliseconds 800
            if (-not $process.HasExited) {
                $sw.Stop()
                $result.cold_start_ms = $sw.ElapsedMilliseconds
                $process.Refresh()
                $result.working_set_mb = [Math]::Round($process.WorkingSet64 / 1MB, 2)
                $cpuStart = $process.TotalProcessorTime.TotalMilliseconds
                $probe = [Diagnostics.Stopwatch]::StartNew()
                Start-Sleep -Seconds ([Math]::Max(10, $IdleSeconds))
                $probe.Stop()
                $process.Refresh()
                $cpuEnd = $process.TotalProcessorTime.TotalMilliseconds
                $cores = [Environment]::ProcessorCount
                $result.idle_cpu_percent = [Math]::Round(
                    (($cpuEnd - $cpuStart) / ($probe.ElapsedMilliseconds * $cores)) * 100, 3)
                $result.working_set_after_idle_mb = [Math]::Round($process.WorkingSet64 / 1MB, 2)
                $result.note = 'Demo mode only. The result does not prove native-service or Bluetooth behavior.'
            }
            else {
                $result.note = "bluu.exe exited during measurement with code $($process.ExitCode)"
            }
            if (-not $process.HasExited) {
                Stop-Process -Id $process.Id -Force
            }
        }
        catch {
            $result.note = $_.Exception.Message
        }
    }
    $result | ConvertTo-Json -Depth 6 | Set-Content -Path $path -Encoding UTF8
}

Write-Evidence "Bluu hardware validation started"
Write-Evidence "root: $BluuRoot"
Write-Evidence "output: $Output"

Save-Command 'system-info.txt' 'systeminfo.exe'
Save-Command 'windows-version.txt' 'cmd.exe' @('/c', 'ver')
Save-Command 'bluetooth-radio.txt' 'powershell.exe' @(
    '-NoProfile', '-Command',
    'Get-PnpDevice -Class Bluetooth -ErrorAction SilentlyContinue | Select-Object Status,Class,FriendlyName,InstanceId | Format-List'
)
Save-Command 'audio-endpoints.txt' 'powershell.exe' @(
    '-NoProfile', '-Command',
    'Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Select-Object Status,FriendlyName,InstanceId | Format-List'
)
Save-Command 'drivers-before.txt' 'pnputil.exe' @('/enum-drivers')
Save-Command 'services.txt' 'sc.exe' @('query', 'BthA2dp')
Save-Command 'services-owb.txt' 'sc.exe' @('query', 'owb-service')

Invoke-Probe 'snapshot.json' 'snapshot'
Invoke-Probe 'aac-state.json' 'aac-state'
Invoke-Probe 'first-scan.json' 'first-scan'
Get-TraceMetadata
Get-PerformanceEvidence

if (-not $SkipProvider) {
    Invoke-Probe 'provider-smoke.json' 'provider-smoke'
}

Save-Command 'drivers-after.txt' 'pnputil.exe' @('/enum-drivers')
Invoke-Probe 'redaction.json' 'redaction'

$summary = @{
    captured_at = (Get-Date).ToString('o')
    host = $env:COMPUTERNAME
    windows = (Get-CimInstance Win32_OperatingSystem).Version
    elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    trace_requested = -not $SkipTrace
    provider_requested = -not $SkipProvider
    output = $Output
    warning = 'This directory contains local hardware evidence. It does not mark any capability verified until a reviewer records an independent trace/playback result in docs/TESTING.md.'
}
$summary | ConvertTo-Json -Depth 6 | Set-Content -Path (Join-Path $Output 'summary.json') -Encoding UTF8

Write-Evidence 'hardware validation evidence collected'
Write-Host "Evidence directory: $Output"
