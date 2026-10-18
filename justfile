# Windows x64 development. Cargo commands run directly; watchexec owns the watcher.
set shell := ["cmd.exe", "/d", "/c"]
set quiet
set minimum-version := "1.59.0"

export PATH := env_var_or_default("CARGO_HOME", home_directory() + "/.cargo") + "/bin;" + env_var("PATH")
export RUSTUP_TOOLCHAIN := "1.95.0"
export CARGO_BUILD_TARGET := "x86_64-pc-windows-msvc"
# Platform scripts use Windows PowerShell 5.1, even when just is launched from pwsh.
export PSModulePath := env_var("SystemRoot") + "/System32/WindowsPowerShell/v1.0/Modules"
# Evaluated once per just invocation, inherited unchanged by every watchexec restart.
export UTILIZER_DEV_SESSION := sha256(uuid() + uuid())

default:
    just --list

# Prepare missing Windows/Rust tools and pinned development commands.
setup:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1

# Preview opens an app window. Use --no-open-pwa to skip it or --auth to test login.
dev *args: _dev-setup
    watchexec --restart --shell=none --debounce=250ms --stop-signal=KILL \
        --workdir "{{justfile_directory()}}" --env UTILIZER_DEV_WATCHER=1 \
        --watch src --watch templates --watch .cargo \
        --watch Cargo.toml --watch Cargo.lock --watch rust-toolchain.toml \
        --watch assets/logo.ico --watch assets/htmx-2.0.8.min.js \
        --watch assets/manifest.webmanifest --watch assets/icon-192.png --watch assets/icon-512.png \
        --watch assets/maps --watch assets/fonts \
        -- cargo run --locked --target-dir target/dev-build --features dev -- --preview {{args}}

_dev-setup:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/setup.ps1 -Preview

# Stop this workspace's development watcher and its child server, from any terminal.
stop:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/stop-dev.ps1

# The same verification pipeline runs locally and in Windows CI.
check: fmt lint test lint-dev test-dev build smoke scripts installer

fmt: setup
    cargo fmt --check

lint: setup
    cargo clippy --locked --all-targets -- -D warnings

test: setup
    cargo test --locked

lint-dev: setup
    cargo clippy --locked --all-targets --features dev -- -D warnings

test-dev: setup
    cargo test --locked --features dev

build: setup
    cargo build --locked --release

smoke: build
    target\x86_64-pc-windows-msvc\release\utilizer.exe --version

scripts:
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests/scripts.ps1

installer: build
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File tests/installer.ps1 -Executable target/x86_64-pc-windows-msvc/release/utilizer.exe
