# Uninstall Utilizer for the current Windows user. Keep playlists by default.
param(
    [Alias('RemoveData')][switch]$Purge,
    [int]$WaitForProcessId = 0,
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This uninstaller requires Windows.'
}
if (-not $env:LOCALAPPDATA -or -not [IO.Path]::IsPathRooted($env:LOCALAPPDATA)) {
    throw 'LOCALAPPDATA must be an absolute directory.'
}
if (-not $CheckOnly -and $WaitForProcessId -gt 0) {
    $parent = Get-Process -Id $WaitForProcessId -ErrorAction SilentlyContinue
    if ($parent -and -not $parent.WaitForExit(15000)) {
        throw 'Utilizer did not exit. Nothing was removed; retry after closing it.'
    }
}
$base = [IO.Path]::GetFullPath($env:LOCALAPPDATA).TrimEnd('\')
$installDirectory = Join-Path $base 'Programs\Utilizer'
$dataDirectory = Join-Path $base 'Utilizer'
$installedExecutable = Join-Path $installDirectory 'utilizer.exe'
if (Get-Process -Name utilizer -ErrorAction SilentlyContinue | Where-Object {
    $_.Id -ne $WaitForProcessId -and (-not $_.Path -or $_.Path -ieq $installedExecutable)
}) {
    throw 'Close the Utilizer console before uninstalling.'
}

# Validate every existing ancestor and target before removing anything. Refuse
# junctions/symlinks. Purge may recursively remove only these two fixed folders.
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
if ($Purge) { $targets = @($installDirectory, $dataDirectory) }
foreach ($target in $targets) {
    Assert-RealPath $target
    if ($Purge -and (Test-Path -LiteralPath $target)) {
        if (-not (Test-Path -LiteralPath $target -PathType Container)) { throw "Expected an application folder: $target" }
        # Walk one level at a time so a nested junction is rejected before any
        # traversal into it. Check the whole tree before changing anything.
        $folders = [Collections.Generic.Queue[string]]::new()
        $folders.Enqueue($target)
        while ($folders.Count) {
            foreach ($item in Get-ChildItem -LiteralPath $folders.Dequeue() -Force) {
                Assert-RealPath $item.FullName
                if ($item.PSIsContainer) { $folders.Enqueue($item.FullName) }
            }
        }
    } elseif ((Test-Path -LiteralPath $target) -and -not (Test-Path -LiteralPath $target -PathType Leaf)) {
        throw "Expected an installed file: $target"
    }
}
if ($CheckOnly) { return }
foreach ($target in $targets) {
    if (Test-Path -LiteralPath $target) {
        if ($Purge) { Remove-Item -LiteralPath $target -Recurse -Force }
        else { Remove-Item -LiteralPath $target -Force }
    }
}
foreach ($directory in @($installDirectory, $dataDirectory)) {
    if (($directory -eq $installDirectory -or $Purge) -and (Test-Path -LiteralPath $directory) -and
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
if ($userPath -and $updatedPath -cne $userPath) {
    [Environment]::SetEnvironmentVariable('Path', $updatedPath, 'User')
}
$env:Path = Without-UtilizerPath $env:Path
Write-Host 'Utilizer uninstalled for this user.'
if ($Purge) { Write-Host 'Application and local data folders removed, including playlists and settings.' }
else { Write-Host 'Playlists and settings kept. Run this script with -Purge to remove them later.' }
Write-Host 'Open a new terminal to refresh PATH.'
Write-Host 'Exported CS2 cfg files and key bindings are kept. Restore your preferred bindings in CS2.'
