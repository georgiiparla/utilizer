# Windows bootstrap only; recipes and file watching belong to just/watchexec.
param([switch]$Preview)
$ErrorActionPreference = 'Stop'

function Invoke-Checked {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed (exit $LASTEXITCODE)" }
}

function Install-LocalCommand {
    param([string]$Command, [string]$Crate, [string]$Version)
    if (Get-Command $Command -ErrorAction SilentlyContinue) {
        $reported = & $Command --version
        if ($LASTEXITCODE -eq 0 -and $reported -match "^$Command $([regex]::Escape($Version))(?:\s|$)") { return }
    }
    Write-Host "Installing $Command $Version..."
    Invoke-Checked cargo @('install', $Crate, '--version', $Version, '--locked', '--force')
}

function Initialize-LocalTools {
    if ([Environment]::OSVersion.Platform -ne 'Win32NT') { throw 'Use Windows for the local development workflow.' }
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $cargoBin = Join-Path $env:USERPROFILE '.cargo/bin'
    if ($env:CARGO_HOME) { $cargoBin = Join-Path $env:CARGO_HOME 'bin' }
    if (Test-Path -LiteralPath $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $tools = @()
    if (Test-Path -LiteralPath $vswhere) {
        $tools = @(& $vswhere -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
    }
    $sdkLibraries = @(Get-ChildItem 'C:/Program Files (x86)/Windows Kits/10/Lib/*/um/x64/kernel32.lib' -ErrorAction SilentlyContinue)
    if ($tools.Count -gt 0 -and $sdkLibraries.Count -eq 0) {
        Write-Host 'Adding the Windows SDK to the existing C++ build tools...'
        $setup = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/setup.exe'
        $workload = if (Test-Path -LiteralPath (Join-Path $tools[0] 'Common7/IDE/devenv.exe')) {
            'Microsoft.VisualStudio.Workload.NativeDesktop'
        } else { 'Microsoft.VisualStudio.Workload.VCTools' }
        $arguments = 'modify --installPath "{0}" --passive --norestart --add {1} --includeRecommended' -f $tools[0], $workload
        $process = Start-Process -FilePath $setup -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -in @(3010, 1641)) { throw 'Windows SDK installed. Restart Windows, then run this command again.' }
        if ($process.ExitCode -ne 0) { throw "Windows SDK installation failed (exit $($process.ExitCode))." }
        if (-not (Get-ChildItem 'C:/Program Files (x86)/Windows Kits/10/Lib/*/um/x64/kernel32.lib' -ErrorAction SilentlyContinue)) {
            throw 'Windows SDK is still unavailable after installation.'
        }
    }
    if ($tools.Count -eq 0) {
        Write-Host 'Installing Microsoft C++ build tools and Windows SDK...'
        $installer = Join-Path ([IO.Path]::GetTempPath()) ('utilizer-buildtools-' + [Guid]::NewGuid().ToString('N') + '.exe')
        try {
            Invoke-WebRequest -UseBasicParsing 'https://aka.ms/vs/17/release/vs_buildtools.exe' -OutFile $installer
            $signature = Get-AuthenticodeSignature -LiteralPath $installer
            if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') {
                throw 'Microsoft build tools installer signature verification failed.'
            }
            $process = Start-Process -FilePath $installer -ArgumentList '--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended' -WindowStyle Hidden -Wait -PassThru
            if ($process.ExitCode -in @(3010, 1641)) { throw 'Build tools installed. Restart Windows, then run this command again.' }
            if ($process.ExitCode -ne 0) { throw "Build tools installation failed (exit $($process.ExitCode))." }
        } finally { Remove-Item -LiteralPath $installer -Force -ErrorAction SilentlyContinue }
        $tools = @(& $vswhere -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
        if ($tools.Count -eq 0) { throw 'C++ build tools are still unavailable after installation.' }
    }
    if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        Write-Host 'Installing Rustup...'
        $installer = Join-Path ([IO.Path]::GetTempPath()) ('utilizer-rustup-' + [Guid]::NewGuid().ToString('N') + '.exe')
        try {
            Invoke-WebRequest -UseBasicParsing 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile $installer
            Invoke-Checked $installer @('-y', '--profile', 'minimal', '--default-toolchain', 'none', '--default-host', 'x86_64-pc-windows-msvc')
        } finally { Remove-Item -LiteralPath $installer -Force -ErrorAction SilentlyContinue }
        $env:PATH = "$cargoBin;$env:PATH"
    }
    $installed = @(& rustup toolchain list)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect installed Rust toolchains.' }
    if (-not ($installed -match '^1\.95\.0-x86_64-pc-windows-msvc(?:\s|$)')) {
        Write-Host 'Installing Rust 1.95.0...'
        Invoke-Checked rustup @('toolchain', 'install', '1.95.0', '--profile', 'minimal', '--component', 'rustfmt', '--component', 'clippy', '--target', 'x86_64-pc-windows-msvc')
    }
    $components = @(& rustup component list --toolchain 1.95.0 --installed)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect Rust components.' }
    foreach ($component in @('rustfmt', 'clippy')) {
        if (-not ($components -match "^$component-")) {
            Invoke-Checked rustup @('component', 'add', '--toolchain', '1.95.0', $component)
        }
    }
    $targets = @(& rustup target list --toolchain 1.95.0 --installed)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect Rust targets.' }
    if ($targets -notcontains 'x86_64-pc-windows-msvc') {
        Invoke-Checked rustup @('target', 'add', '--toolchain', '1.95.0', 'x86_64-pc-windows-msvc')
    }
    $env:RUSTUP_TOOLCHAIN = '1.95.0'
    $env:CARGO_BUILD_TARGET = 'x86_64-pc-windows-msvc'
    Write-Host 'Local build tools ready (Rust 1.95.0, Windows x64).'
}

Initialize-LocalTools
Install-LocalCommand 'just' 'just' '1.59.0'
Install-LocalCommand 'watchexec' 'watchexec-cli' '2.8.0'
if ($Preview) {
    # Refuse a second server before entering the long-running watchexec loop.
    $probe = New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback, 47825)
    try { $probe.Start() } catch { throw 'Port 47825 is occupied. Close the running Utilizer instance first.' }
    finally { $probe.Stop() }
}
