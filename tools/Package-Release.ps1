param([string]$Executable = 'target/release/pimpit.exe')
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$version = (Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected stable three-component version.' }
if (-not [IO.Path]::IsPathRooted($Executable)) { $Executable = Join-Path $root $Executable }
$output = Join-Path $root 'release'
$stage = Join-Path ([IO.Path]::GetTempPath()) ('Pimpit-package-' + [guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($stage)
[void][IO.Directory]::CreateDirectory($output)
try {
    Copy-Item -LiteralPath $Executable -Destination (Join-Path $stage 'Pimpit.exe')
    foreach ($name in @('pimpit.ini', 'README-TH.md', 'VALIDATION-TH.md', 'GITHUB-TH.md', 'LICENSE')) {
        Copy-Item -LiteralPath (Join-Path $root $name) -Destination $stage
    }
    [void][IO.Directory]::CreateDirectory((Join-Path $stage 'licenses'))
    foreach ($name in @('LICENSE-ENGLISH', 'LICENSE-THAI.md', 'LICENSE-PYTHAINLP', 'SOURCES.md')) {
        Copy-Item -LiteralPath (Join-Path $root "data/$name") -Destination (Join-Path $stage 'licenses')
    }
    $archive = Join-Path $output "Pimpit-$version-windows-x64.zip"
    Get-ChildItem -LiteralPath $stage | Compress-Archive -DestinationPath $archive -Force
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$archive.sha256", "$hash  $([IO.Path]::GetFileName($archive))`n", [Text.Encoding]::ASCII)
    Write-Output $archive
} finally {
    # Delete only known files, after resolving the unique temp directory.
    foreach ($name in @('pimpit.ini', 'README-TH.md', 'VALIDATION-TH.md', 'GITHUB-TH.md', 'LICENSE', 'Pimpit.exe')) {
        $file = Join-Path $stage $name
        if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file }
    }
    $licenses = Join-Path $stage 'licenses'
    if (Test-Path -LiteralPath $licenses) { Get-ChildItem -LiteralPath $licenses -File | Remove-Item; Remove-Item -LiteralPath $licenses }
    Remove-Item -LiteralPath $stage
}
