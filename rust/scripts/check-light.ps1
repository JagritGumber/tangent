param(
    [string]$Test = ""
)

$ErrorActionPreference = "Stop"

$workspace = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $workspace
try {
    $env:CARGO_BUILD_JOBS = "1"
    $env:CARGO_INCREMENTAL = "0"

    cargo fmt --check
    cargo metadata --no-deps --format-version 1 | Out-Null

    if ($Test -ne "") {
        cargo test -p tangent-sdk $Test
    }
}
finally {
    Pop-Location
}
