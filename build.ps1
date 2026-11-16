param([switch]$Test, [switch]$NativeTest, [switch]$BuildOnly)
$ErrorActionPreference = 'Stop'
Push-Location -LiteralPath $PSScriptRoot
try {
    # Use the already installed self-contained GNU Rust linker, no Visual Studio needed.
    $rustExecutable = (& rustup which rustc --toolchain stable-x86_64-pc-windows-gnu).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Install stable-x86_64-pc-windows-gnu using rustup.' }
    $rustRoot = Split-Path (Split-Path $rustExecutable -Parent) -Parent
    $rustTools = Join-Path $rustRoot 'lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained'
    $env:PATH = $rustTools + ';' + $env:PATH
    # GNU dlltool also needs the MinGW assembler. Override for another machine.
    $mingwBin = if ($env:PIMPIT_MINGW_BIN) { $env:PIMPIT_MINGW_BIN } else { Join-Path $PSScriptRoot '../savehaven-rust/.tools/w64devkit/bin' }
    if (Test-Path -LiteralPath $mingwBin) { $env:PATH = $mingwBin + ';' + $env:PATH }
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $rustTools 'x86_64-w64-mingw32-gcc.exe'
    if ($Test) {
        & cargo +stable-x86_64-pc-windows-gnu test --locked --offline
        if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
    }
    if ($NativeTest) {
        & cargo +stable-x86_64-pc-windows-gnu test --locked --offline native_ -- --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw 'Native integration test failed.' }
    }
    & cargo +stable-x86_64-pc-windows-gnu build --release --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    if ($BuildOnly) { Write-Output (Join-Path $PSScriptRoot 'target/release/pimpit.exe'); return }
    New-Item -ItemType Directory -Path dist/licenses -Force | Out-Null
    Copy-Item -LiteralPath target/release/pimpit.exe -Destination dist/Pimpit.exe
    Copy-Item -LiteralPath README-TH.md,VALIDATION-TH.md,GITHUB-TH.md,LICENSE -Destination dist
    if (-not (Test-Path -LiteralPath dist/pimpit.ini)) {
        if (Test-Path -LiteralPath dist/rightlang.ini) { Copy-Item -LiteralPath dist/rightlang.ini -Destination dist/pimpit.ini }
        else { Copy-Item -LiteralPath pimpit.ini -Destination dist }
    }
    Copy-Item -LiteralPath data/LICENSE-ENGLISH,data/LICENSE-THAI.md,data/LICENSE-PYTHAINLP -Destination dist/licenses
    Copy-Item -LiteralPath data/SOURCES.md -Destination dist/licenses
    Write-Output (Join-Path $PSScriptRoot 'dist/Pimpit.exe')
} finally { Pop-Location }
