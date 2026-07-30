# The full check suite. Run this after completing a piece of work (CLAUDE.md).
#
#   .\scripts\check.ps1
#
# Exits 0 if everything passed, 1 on the first failure. Safe to run from any directory.

# cargo is not on PATH in a fresh shell on this machine.
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# Locate the workspace root from the script's own location, so the caller's working
# directory is never changed.
$manifest = Join-Path (Split-Path -Parent $PSScriptRoot) 'Cargo.toml'

function Invoke-Check {
    param([Parameter(Mandatory)][string[]]$CargoArgs)

    Write-Host "==> cargo $($CargoArgs -join ' ')" -ForegroundColor Cyan
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: cargo $($CargoArgs -join ' ') (exit $LASTEXITCODE)" -ForegroundColor Red
        exit 1
    }
}

# `--all` is required. Without it, `cargo fmt --manifest-path` against a *virtual*
# workspace manifest fails with "Failed to find targets", because the root has none.
Invoke-Check @('fmt', '--all', '--manifest-path', $manifest, '--check')

# `--all-targets` so integration tests are linted too, and `-D warnings` because plain
# `cargo clippy` exits 0 even after printing warnings.
Invoke-Check @('clippy', '--manifest-path', $manifest, '--workspace', '--all-targets', '--', '-D', 'warnings')

Invoke-Check @('test', '--manifest-path', $manifest, '--workspace')

# The release run is not redundant: `overflow-checks = true` exists only in the release
# profile, and cargo silently ignores `[profile.*]` if it is ever moved out of the
# workspace-root manifest. No test-name filter, because a filter that matches nothing
# exits 0 -- which would turn this guard into a silent no-op after a rename.
Invoke-Check @('test', '--manifest-path', $manifest, '--workspace', '--release')

Write-Host 'All checks passed.' -ForegroundColor Green
