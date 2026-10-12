$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$nodeExecutable = Join-Path $projectRoot '.tools\node-v22.23.3-win-x64\node.exe'
if (-not (Test-Path -LiteralPath $nodeExecutable)) { throw 'Node local não encontrado em .tools.' }
Set-Location (Join-Path $projectRoot 'apps\web')
& $nodeExecutable 'node_modules\vite\bin\vite.js' '--host' '127.0.0.1' '--port' '5173' '--strictPort'
exit $LASTEXITCODE
