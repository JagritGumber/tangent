param(
    [string]$Test = "",
    [switch]$Clippy
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$workspace = Resolve-Path (Join-Path $PSScriptRoot "..")
$previousBuildJobs = $env:CARGO_BUILD_JOBS
$previousIncremental = $env:CARGO_INCREMENTAL

Push-Location $workspace
try {
    $env:CARGO_BUILD_JOBS = "1"
    $env:CARGO_INCREMENTAL = "0"

    cargo fmt --check
    cargo metadata --no-deps --format-version 1 | Out-Null

    if ($Clippy) {
        cargo clippy -p tangent-sdk --all-targets -- -D warnings
    }

    if ($Test -ne "") {
        cargo test -p tangent-sdk $Test
    }
}
finally {
    Pop-Location

    if ($null -eq $previousBuildJobs) {
        Remove-Item Env:\CARGO_BUILD_JOBS -ErrorAction SilentlyContinue
    }
    else {
        $env:CARGO_BUILD_JOBS = $previousBuildJobs
    }

    if ($null -eq $previousIncremental) {
        Remove-Item Env:\CARGO_INCREMENTAL -ErrorAction SilentlyContinue
    }
    else {
        $env:CARGO_INCREMENTAL = $previousIncremental
    }
}
