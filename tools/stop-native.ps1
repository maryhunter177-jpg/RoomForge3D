$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root '.tools'
$stateFile = Join-Path $tools 'native-processes.json'
$data = Join-Path $tools 'postgres-data-native'
$pgBin = Join-Path $tools 'pgsql\bin'

if (Test-Path -LiteralPath $stateFile) {
  $state = Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json
  if ([IO.Path]::GetFullPath($state.data) -ne [IO.Path]::GetFullPath($data)) { throw 'Estado aponta para outro cluster; parada cancelada.' }
  foreach ($record in $state.processes) {
    $running = Get-Process -Id $record.id -ErrorAction SilentlyContinue
    if ($null -eq $running) { continue }
    $expected = [IO.Path]::GetFullPath($record.executable)
    if ($expected -ne (Join-Path $root 'target\debug\roomforge-api.exe') -and $expected -ne (Join-Path $tools 'node-v22.23.3-win-x64\node.exe') -and $expected -ne (Join-Path $tools 'minio\minio.exe')) { throw 'Executavel inesperado no estado; parada cancelada.' }
    if ($running.Path -ne $expected) { throw "PID $($record.id) pertence a outro executavel; parada cancelada." }
    if ($running.StartTime.ToUniversalTime().ToString('o') -ne $record.started) { throw "PID $($record.id) foi reutilizado; parada cancelada." }
    Stop-Process -Id $record.id
  }
}
if (Test-Path -LiteralPath (Join-Path $data 'postmaster.pid')) {
  $pidLines = Get-Content -LiteralPath (Join-Path $data 'postmaster.pid')
  if ($pidLines.Count -lt 3 -or [IO.Path]::GetFullPath($pidLines[1]) -ne [IO.Path]::GetFullPath($data)) { throw 'Arquivo PID aponta para outro cluster; parada cancelada.' }
  $postgresPid = [int]$pidLines[0]
  $postgres = Get-Process -Id $postgresPid -ErrorAction SilentlyContinue
  if ($postgres) {
    if ($postgres.Path -ne (Join-Path $pgBin 'postgres.exe')) { throw 'PID PostgreSQL pertence a outro executavel; parada cancelada.' }
    $startedUnix = [DateTimeOffset]::new($postgres.StartTime.ToUniversalTime()).ToUnixTimeSeconds()
    if ([Math]::Abs($startedUnix - [long]$pidLines[2]) -gt 5) { throw 'PID PostgreSQL foi reutilizado; parada cancelada.' }
  }
  if ($postgres) {
  $pgControl = Start-Process -FilePath (Join-Path $pgBin 'pg_ctl.exe') -ArgumentList @('-D', ('"' + $data + '"'), '-m', 'fast', '-t', '60', '-w', 'stop') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $tools 'native-stop-postgres.log') -RedirectStandardError (Join-Path $tools 'native-stop-postgres-error.log')
  $null = $pgControl.Handle
  $pgDeadline = [DateTime]::UtcNow.AddSeconds(60)
  while (-not $pgControl.WaitForExit(1000)) {
    if ([DateTime]::UtcNow -ge $pgDeadline) { throw 'Tempo limite de parada PostgreSQL excedido.' }
  }
  $pgControl.Refresh()
  if ($null -eq $pgControl.ExitCode -or $pgControl.ExitCode -ne 0) { throw 'Falha ao parar cluster PostgreSQL isolado.' }
  } else { Write-Output 'PID PostgreSQL antigo sem processo; arquivo preservado para verificacao pelo PostgreSQL no proximo inicio.' }
}
if (Test-Path -LiteralPath $stateFile) { Remove-Item -LiteralPath $stateFile }
Write-Output 'Processos nativos RoomForge encerrados. Dados e segredos locais preservados.'




