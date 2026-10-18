# Uninstall Utilizer for the current Windows user. Keep playlists by default.
param([switch]$RemoveData)
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This uninstaller requires Windows.'
}
if (-not $env:LOCALAPPDATA -or -not [IO.Path]::IsPathRooted($env:LOCALAPPDATA)) {
    throw 'LOCALAPPDATA must be an absolute directory.'
}
if (Get-Process -Name utilizer -ErrorAction SilentlyContinue) {
    throw 'Close the Utilizer console before uninstalling.'
}
$base = [IO.Path]::GetFullPath($env:LOCALAPPDATA).TrimEnd('\')
$installDirectory = Join-Path $base 'Programs\Utilizer'
$dataDirectory = Join-Path $base 'Utilizer'

# Validate every existing ancestor and target before removing anything. Refuse
# junctions/symlinks, and remove known files only; never recursively delete a folder.
function Assert-RealPath([string]$Path) {
    $candidate = [IO.Path]::GetFullPath($Path)
    if (-not $candidate.StartsWith($base + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Uninstall target is outside LOCALAPPDATA.'
    }
    while ($candidate) {
        if (Test-Path -LiteralPath $candidate) {
            $item = Get-Item -LiteralPath $candidate -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Refusing to uninstall through a link: $candidate"
            }
        }
        $candidate = [IO.Path]::GetDirectoryName($candidate)
    }
}
$targets = @((Join-Path $installDirectory 'utilizer.exe'))
if ($RemoveData) {
    $targets += @('utilizer.sqlite3', 'utilizer.sqlite3-journal', 'utilizer.sqlite3-wal', 'utilizer.sqlite3-shm', 'instance.lock') |
        ForEach-Object { Join-Path $dataDirectory $_ }
}
foreach ($target in $targets) {
    Assert-RealPath $target
    if ((Test-Path -LiteralPath $target) -and -not (Test-Path -LiteralPath $target -PathType Leaf)) {
        throw "Expected an installed file: $target"
    }
}
foreach ($target in $targets) {
    if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Force }
}
foreach ($directory in @($installDirectory, $dataDirectory)) {
    if (($directory -eq $installDirectory -or $RemoveData) -and (Test-Path -LiteralPath $directory) -and
        @(Get-ChildItem -LiteralPath $directory -Force).Count -eq 0) {
        Remove-Item -LiteralPath $directory -Force
    }
}
function Without-UtilizerPath([string]$Value) {
    (@($Value -split ';' | Where-Object {
        $_.Trim().Trim('"').TrimEnd('\') -ine $installDirectory
    }) -join ';')
}
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$updatedPath = Without-UtilizerPath $userPath
if ($updatedPath -cne $userPath) {
    [Environment]::SetEnvironmentVariable('Path', $updatedPath, 'User')
}
$env:Path = Without-UtilizerPath $env:Path
Write-Host 'Utilizer uninstalled for this user.'
if ($RemoveData) { Write-Host 'Local playlists and settings removed.' }
else { Write-Host 'Playlists and settings kept. Use -RemoveData to remove them too.' }
Write-Host 'Exported CS2 cfg files and key bindings are kept. Restore your preferred bindings in CS2.'
