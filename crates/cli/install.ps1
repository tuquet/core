<#
.SYNOPSIS
    Operon CLI Installer (Windows x64)
.DESCRIPTION
    Downloads operon.exe, verifies SHA256, installs operon.exe + opn.exe alias
    to %USERPROFILE%\.operon\bin, registers daemon autostart, and starts the daemon.
#>
param(
    [switch]$NoDaemon
)

$ErrorActionPreference = "Stop"

$Repo = if ($env:OPERON_REPO) { $env:OPERON_REPO } else { "tuquet/operon" }
$AssetName = "operon-windows-x64.exe"
$BaseUrl = "https://github.com/$Repo/releases/latest/download"
$OperonHome = if ($env:OPERON_HOME) { $env:OPERON_HOME } else { Join-Path $env:USERPROFILE ".operon" }
$BinDir = Join-Path $OperonHome "bin"
$TargetExe = Join-Path $BinDir "operon.exe"
$AliasExe = Join-Path $BinDir "opn.exe"
$TaskName = "OperonDaemon"
$LegacyVbs = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\Startup\OperonLocalBridge.vbs"

Write-Host "[operon] Installing Operon CLI to $BinDir..." -ForegroundColor Cyan
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("operon-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    $TmpExe = Join-Path $TmpDir $AssetName
    $SumsFile = Join-Path $TmpDir "SHA256SUMS.txt"

    Write-Host "[operon] Downloading $AssetName..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri "$BaseUrl/$AssetName" -OutFile $TmpExe -UseBasicParsing

    $HasSums = $false
    try {
        Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS.txt" -OutFile $SumsFile -UseBasicParsing
        $HasSums = $true
    } catch {
        Write-Host "[operon] Warning: SHA256SUMS.txt unavailable ($($_.Exception.Message))." -ForegroundColor Yellow
    }

    if ($HasSums) {
        $ExpectedLine = Get-Content $SumsFile | Where-Object { $_ -match "\s+\*?$([regex]::Escape($AssetName))$" } | Select-Object -First 1
        if (-not $ExpectedLine) {
            throw "SHA256 entry missing for $AssetName in SHA256SUMS.txt"
        }
        $ExpectedHash = ($ExpectedLine -split '\s+')[0].ToLowerInvariant()
        $ActualHash = (Get-FileHash -Path $TmpExe -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($ActualHash -ne $ExpectedHash) {
            throw "SHA256 mismatch for $AssetName (expected $ExpectedHash, got $ActualHash)"
        }
        Write-Host "[operon] SHA256 verified." -ForegroundColor Green
    }

    # Stop active daemon and wait until binary handles close before swapping
    foreach ($Candidate in @($TargetExe, $AliasExe)) {
        if (Test-Path $Candidate) {
            try { & $Candidate daemon stop --force 2>$null | Out-Null } catch {}
        }
    }
    for ($i = 0; $i -lt 20; $i++) {
        $Running = Get-Process -Name "operon", "opn" -ErrorAction SilentlyContinue
        if (-not $Running) { break }
        Start-Sleep -Milliseconds 250
    }

    Move-Item -Path $TmpExe -Destination $TargetExe -Force
    Remove-Item -Path $AliasExe -Force -ErrorAction SilentlyContinue
    try {
        New-Item -ItemType HardLink -Path $AliasExe -Target $TargetExe -ErrorAction Stop | Out-Null
    } catch {
        Copy-Item -Path $TargetExe -Destination $AliasExe -Force
    }
    Write-Host "[operon] Installed binary: $TargetExe (alias: opn.exe)" -ForegroundColor Green
} finally {
    Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}

# Add to User PATH idempotently
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
$PathEntries = if ($UserPath) { $UserPath -split ';' | Where-Object { $_ -ne '' } } else { @() }
if (($PathEntries | ForEach-Object { $_.TrimEnd('\') }) -notcontains $BinDir.TrimEnd('\')) {
    $NewPath = (@($PathEntries) + $BinDir) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = "$BinDir;$env:Path"
    Write-Host "[operon] Added $BinDir to User PATH." -ForegroundColor Green
} else {
    $env:Path = "$BinDir;$env:Path"
}

if (Test-Path $LegacyVbs) {
    Remove-Item -Path $LegacyVbs -Force -ErrorAction SilentlyContinue
}

if (-not $NoDaemon) {
    $DaemonArgs = "daemon start -H 127.0.0.1 -p 8765 -d"
    try {
        $Action = New-ScheduledTaskAction -Execute $TargetExe -Argument $DaemonArgs
        $Trigger = New-ScheduledTaskTrigger -AtLogOn
        $Settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew
        Register-ScheduledTask -TaskName $TaskName -Action $Action -Trigger $Trigger -Settings $Settings -Description "Operon Daemon (127.0.0.1:8765)" -Force -ErrorAction Stop | Out-Null
        Write-Host "[operon] Daemon autostart Scheduled Task registered: $TaskName" -ForegroundColor Green
    } catch {
        $RunKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
        if (-not (Test-Path $RunKey)) { New-Item -Path $RunKey -Force | Out-Null }
        Set-ItemProperty -Path $RunKey -Name $TaskName -Value "`"$TargetExe`" $DaemonArgs"
        Write-Host "[operon] Daemon autostart registered via HKCU Run key: $TaskName" -ForegroundColor Green
    }

    Write-Host "[operon] Starting daemon on 127.0.0.1:8765..." -ForegroundColor Cyan
    & $TargetExe daemon start -H 127.0.0.1 -p 8765 -d

    $Healthy = $false
    for ($i = 0; $i -lt 20; $i++) {
        try {
            $Resp = Invoke-RestMethod -Uri "http://127.0.0.1:8765/api/v1/health" -TimeoutSec 2
            if ($Resp.status -eq "ok") {
                $Healthy = $true
                break
            }
        } catch {}
        Start-Sleep -Milliseconds 500
    }

    if ($Healthy) {
        Write-Host "[operon] Daemon online at http://127.0.0.1:8765" -ForegroundColor Green
    } else {
        Write-Host "[operon] Warning: binary installed, but daemon health check on 127.0.0.1:8765 did not respond yet. Run 'opn daemon status' to inspect." -ForegroundColor Yellow
    }
}

$InstalledVer = (& $TargetExe --version) 2>$null
if ($InstalledVer) {
    Write-Host "[operon] Ready: $InstalledVer (run 'opn desk' to open Operon Desktop)" -ForegroundColor Cyan
}
