param([Parameter(Mandatory=$true)][string]$Destination)
$ErrorActionPreference = 'Stop'
$encoded = Join-Path $PSScriptRoot '../data/dictionaries.b64'
[void][IO.Directory]::CreateDirectory($Destination)
$archive = Join-Path $Destination 'dictionaries.zip'
[IO.File]::WriteAllBytes($archive, [Convert]::FromBase64String([IO.File]::ReadAllText($encoded)))
Expand-Archive -LiteralPath $archive -DestinationPath $Destination -Force
$expected = @{
    'english.txt' = '883CDF1AD79DC10B3CEFDBE8D50551280E5E2790D8C30B77E99DC34A7F69D977'
    'thai.txt' = 'D458027AE734D663287290830F6903AF4BF995348CA4C36017F9A62D28DDF061'
}
foreach ($name in $expected.Keys) {
    $stream = [IO.File]::OpenRead((Join-Path $Destination $name))
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $hash = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '') }
    finally { $stream.Dispose(); $sha.Dispose() }
    if ($hash -ne $expected[$name]) {
        throw "Dictionary integrity check failed: $name"
    }
}
