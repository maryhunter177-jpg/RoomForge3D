$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$node = Join-Path $root '.tools\node-v22.23.3-win-x64\node.exe'
& $node (Join-Path $PSScriptRoot 'check-native.mjs')
exit $LASTEXITCODE
