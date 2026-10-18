# Windows process control only. watchexec still owns watching and server restarts.
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path.Replace('\', '/').TrimEnd('/')
$escaped = [regex]::Escape($workspace)
$workdirPattern = '(?:^|\s)--workdir(?:=|\s+)(?:"' + $escaped + '"|' + $escaped + '(?=\s|$))'
$markerPattern = '(?:^|\s)--env(?:=|\s+)"?UTILIZER_DEV_WATCHER=1"?(?=\s|$)'
$watchers = @(Get-CimInstance Win32_Process -Filter "Name='watchexec.exe'" | Where-Object {
    $command = ([string]$_.CommandLine).Replace('\', '/')
    $command -match $workdirPattern -and $command -match $markerPattern
})
if ($watchers.Count -eq 0) {
    Write-Host 'Development preview is already stopped.'
    exit 0
}
foreach ($watcher in $watchers) {
    # Recheck identity so stale enumeration data cannot target a reused PID.
    $current = Get-CimInstance Win32_Process -Filter "ProcessId=$($watcher.ProcessId)"
    if (-not $current -or $current.CreationDate -ne $watcher.CreationDate -or
        $current.ExecutablePath -ne $watcher.ExecutablePath -or
        $current.CommandLine -ne $watcher.CommandLine) { continue }
    $killer = Start-Process -FilePath (Join-Path $env:SystemRoot 'System32/taskkill.exe') -ArgumentList @('/PID', $watcher.ProcessId, '/T', '/F') -WindowStyle Hidden -Wait -PassThru
    if ($killer.ExitCode -ne 0) {
        $remaining = Get-CimInstance Win32_Process -Filter "ProcessId=$($watcher.ProcessId)"
        if ($remaining -and $remaining.CreationDate -eq $watcher.CreationDate) {
            throw 'Cannot stop the development watcher. Close its terminal with Ctrl+C.'
        }
    }
}
Write-Host 'Development watcher and preview stopped.'
