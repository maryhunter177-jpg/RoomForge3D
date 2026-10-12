param([switch]$Build)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root '.tools'
$data = Join-Path $tools 'postgres-data-native'
$envFile = Join-Path $root '.env.native'
$stateFile = Join-Path $tools 'native-processes.json'
$pgBin = Join-Path $tools 'pgsql\bin'

$node = Join-Path $tools 'node-v22.23.3-win-x64\node.exe'
$api = Join-Path $root 'target\debug\roomforge-api.exe'
$minio = Join-Path $tools 'minio\minio.exe'
$storageSetup = Join-Path $root 'tools\setup-storage.mjs'
$vite = Join-Path $root 'apps\web\node_modules\vite\bin\vite.js'
foreach ($file in @((Join-Path $pgBin 'initdb.exe'), (Join-Path $pgBin 'pg_ctl.exe'), (Join-Path $pgBin 'psql.exe'), $node, $minio, $storageSetup)) {
  if (-not (Test-Path -LiteralPath $file)) { throw "Dependencia ausente: $file" }
}
if (Test-Path -LiteralPath $stateFile) { throw 'Estado nativo existente. Execute tools/stop-native.ps1 antes de iniciar novamente.' }
foreach ($port in @(55432, 8080, 5173, 9000, 9001)) {
  if (Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue) { throw "Porta $port ocupada; nenhum processo existente sera alterado." }
}
New-Item -ItemType Directory -Force -Path $tools | Out-Null
function New-Secret {
  $bytes = New-Object byte[] 32
  $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
  try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
  return ([BitConverter]::ToString($bytes)).Replace('-', '').ToLowerInvariant()
}
if (-not (Test-Path -LiteralPath $envFile)) {
  $password = New-Secret
  $jwt = New-Secret
  @("DATABASE_URL=postgres://roomforge:${password}@127.0.0.1:55432/roomforge", "JWT_SECRET=$jwt", 'API_HOST=127.0.0.1', 'API_PORT=8080', 'CORS_ORIGIN=http://localhost:5173', 'RUST_LOG=roomforge_api=info,tower_http=info') | Set-Content -LiteralPath $envFile -Encoding ascii
}
$nativeEntries = Get-Content -LiteralPath $envFile
$defaults = [ordered]@{
  STORAGE_ENDPOINT = 'http://127.0.0.1:9000'
  STORAGE_REGION = 'us-east-1'
  STORAGE_BUCKET = 'roomforge'
  STORAGE_ACCESS_KEY = ('roomforge' + (New-Secret).Substring(0,16))
  STORAGE_SECRET_KEY = (New-Secret)
  NODE_BINARY = $node
  OPTIMIZER_SCRIPT = (Join-Path $root 'tools\optimize-model.mjs')
  ADMIN_EMAIL = ''
}
foreach ($key in $defaults.Keys) {
  if (-not ($nativeEntries | Where-Object { $_.StartsWith($key + '=') })) { Add-Content -LiteralPath $envFile -Value ($key + '=' + $defaults[$key]) -Encoding ascii }
}
$oldEnvironment = @{}
$postgresStarted = $false
try {
  $nativePaths = [ordered]@{
    TEMP = (Join-Path $tools 'tmp')
    TMP = (Join-Path $tools 'tmp')
    TMPDIR = (Join-Path $tools 'tmp')
    NPM_CONFIG_CACHE = (Join-Path $tools 'npm-cache')
    CARGO_TARGET_DIR = (Join-Path $root 'target')
  }
  foreach ($key in $nativePaths.Keys) {
    New-Item -ItemType Directory -Force -Path $nativePaths[$key] | Out-Null
    $oldEnvironment[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
    [Environment]::SetEnvironmentVariable($key, $nativePaths[$key], 'Process')
  }
  foreach ($line in Get-Content -LiteralPath $envFile) {
    if ($line -match '^([A-Z_]+)=(.*)$') {
      $key = $Matches[1]; $value = $Matches[2]
      $oldEnvironment[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
      [Environment]::SetEnvironmentVariable($key, $value, 'Process')
    }
  }
  $databaseUri = [Uri]$env:DATABASE_URL
  if ($databaseUri.Host -ne '127.0.0.1' -or $databaseUri.Port -ne 55432 -or $databaseUri.AbsolutePath -ne '/roomforge') { throw '.env.native deve apontar para o banco isolado 127.0.0.1:55432/roomforge.' }
  if ($env:API_HOST -ne '127.0.0.1' -or $env:API_PORT -ne '8080') { throw 'API nativa deve usar 127.0.0.1:8080.' }
  $password = [Uri]::UnescapeDataString(($databaseUri.UserInfo -split ':',2)[1])
  $oldEnvironment['PATH'] = $env:PATH
  $env:PATH = (Split-Path $node) + ';' + (Join-Path $tools 'cargo\bin') + ';' + (Join-Path $tools 'ktx\bin') + ';' + $env:PATH
  if ($Build) {
    $vcvars = Join-Path $tools 'vs-buildtools\VC\Auxiliary\Build\vcvars64.bat'
    if (Test-Path -LiteralPath $vcvars) {
      $envCmd = Join-Path $tools 'native-msvc-env.cmd'
      try {
        @('@echo off', ('call "' + $vcvars + '" >nul'), 'if errorlevel 1 exit /b 1', 'set') | Set-Content -LiteralPath $envCmd -Encoding ascii
        $compilerEnvironment = & $env:ComSpec /d /c ('"' + $envCmd + '"')
        if ($LASTEXITCODE -ne 0) { throw 'Falha ao configurar compilador MSVC local.' }
        foreach ($entry in $compilerEnvironment) {
          $separator = $entry.IndexOf('=')
          if ($separator -le 0) { continue }
          $key = $entry.Substring(0, $separator)
          $value = $entry.Substring($separator + 1)
          if (-not $oldEnvironment.ContainsKey($key)) { $oldEnvironment[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
          [Environment]::SetEnvironmentVariable($key, $value, 'Process')
        }
      } finally { if (Test-Path -LiteralPath $envCmd) { Remove-Item -LiteralPath $envCmd } }
    }
    $oldEnvironment['CARGO_HOME'] = $env:CARGO_HOME; $oldEnvironment['RUSTUP_HOME'] = $env:RUSTUP_HOME
    $env:CARGO_HOME = Join-Path $tools 'cargo'; $env:RUSTUP_HOME = Join-Path $tools 'rustup'
    Push-Location $root
    try {
      & (Join-Path $env:CARGO_HOME 'bin\cargo.exe') build -p roomforge-api
      if ($LASTEXITCODE -ne 0) { throw 'Compilacao da API falhou.' }
      Push-Location (Join-Path $root 'apps\web')
      try {
        & $node (Join-Path (Split-Path $node) 'node_modules\npm\bin\npm-cli.js') ci
        if ($LASTEXITCODE -ne 0) { throw 'Instalacao frontend falhou.' }
        & $node (Join-Path (Split-Path $node) 'node_modules\npm\bin\npm-cli.js') run build
        if ($LASTEXITCODE -ne 0) { throw 'Compilacao frontend falhou.' }
      } finally { Pop-Location }
      Push-Location (Join-Path $root 'tools')
      try {
        & $node (Join-Path (Split-Path $node) 'node_modules\npm\bin\npm-cli.js') ci
        if ($LASTEXITCODE -ne 0) { throw 'Instalacao ferramentas de assets falhou.' }
      } finally { Pop-Location }
    } finally { Pop-Location }
  }
  foreach ($file in @($api, $vite)) { if (-not (Test-Path -LiteralPath $file)) { throw "Build ausente: $file. Execute com -Build." } }
  if (-not (Test-Path -LiteralPath (Join-Path $data 'PG_VERSION'))) {
    $pwFile = Join-Path $tools 'native-init-password.tmp'
    try {
      Set-Content -LiteralPath $pwFile -Value $password -Encoding ascii
      & (Join-Path $pgBin 'initdb.exe') -D $data -U roomforge --auth=scram-sha-256 --pwfile=$pwFile --encoding=UTF8 --locale=C > (Join-Path $tools 'native-initdb.log') 2>&1
      if ($LASTEXITCODE -ne 0) { throw 'initdb falhou. Consulte .tools/native-initdb.log.' }
    } finally { if (Test-Path -LiteralPath $pwFile) { Remove-Item -LiteralPath $pwFile -Force } }
    Add-Content -LiteralPath (Join-Path $data 'postgresql.conf') -Value "`nlisten_addresses = '127.0.0.1'`nport = 55432"
  }
  $pgControl = Start-Process -FilePath (Join-Path $pgBin 'pg_ctl.exe') -ArgumentList @('-D', ('"' + $data + '"'), '-l', ('"' + (Join-Path $tools 'native-postgres.log') + '"'), '-t', '180', '-w', 'start') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $tools 'native-pgctl.log') -RedirectStandardError (Join-Path $tools 'native-pgctl-error.log')
  $postgresStarted = $true
  $null = $pgControl.Handle
  $pgDeadline = [DateTime]::UtcNow.AddSeconds(180)
  while (-not $pgControl.WaitForExit(1000)) {
    if ([DateTime]::UtcNow -ge $pgDeadline) { throw 'Tempo limite de pg_ctl excedido; consulte logs PostgreSQL.' }
  }
  $pgControl.Refresh()
  if ($null -eq $pgControl.ExitCode -or $pgControl.ExitCode -ne 0) { throw 'PostgreSQL nativo nao iniciou. Consulte .tools/native-pgctl.log.' }
  $postgresStarted = $true
  $oldEnvironment['PGPASSWORD'] = $env:PGPASSWORD; $env:PGPASSWORD = $password
  $exists = & (Join-Path $pgBin 'psql.exe') -h 127.0.0.1 -p 55432 -U roomforge -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = 'roomforge'" 2> (Join-Path $tools 'native-psql.log')
  if ($LASTEXITCODE -ne 0) { throw 'Falha ao consultar o banco isolado.' }
  if (($exists -join '').Trim() -ne '1') {
    & (Join-Path $pgBin 'psql.exe') -h 127.0.0.1 -p 55432 -U roomforge -d postgres -c 'CREATE DATABASE roomforge' > (Join-Path $tools 'native-createdb.log') 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Falha ao criar banco roomforge.' }
  }
  $records = @()
  @{ data = $data; pgBin = $pgBin; processes = $records } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $stateFile -Encoding utf8
  foreach ($key in @('MINIO_ROOT_USER','MINIO_ROOT_PASSWORD','MINIO_BROWSER_REDIRECT_URL','MINIO_API_CORS_ALLOW_ORIGIN')) { $oldEnvironment[$key] = [Environment]::GetEnvironmentVariable($key, 'Process') }
  if ([Uri]$env:STORAGE_ENDPOINT -ne [Uri]'http://127.0.0.1:9000') { throw 'Storage nativo deve usar endpoint isolado http://127.0.0.1:9000.' }
  $env:MINIO_ROOT_USER = $env:STORAGE_ACCESS_KEY
  $env:MINIO_ROOT_PASSWORD = $env:STORAGE_SECRET_KEY
  $env:MINIO_BROWSER_REDIRECT_URL = 'http://127.0.0.1:9001'
  $env:MINIO_API_CORS_ALLOW_ORIGIN = 'http://localhost:5173'
  $minioData = Join-Path $tools 'minio-data-native'
  $minioConfig = Join-Path $tools 'minio-config'
  $minioCerts = Join-Path $tools 'minio-certs'
  New-Item -ItemType Directory -Force -Path $minioData,$minioConfig,$minioCerts | Out-Null
  $minioProcess = Start-Process -FilePath $minio -ArgumentList @('--config-dir', ('"' + $minioConfig + '"'), '--certs-dir', ('"' + $minioCerts + '"'), 'server', ('"' + $minioData + '"'), '--address', '127.0.0.1:9000', '--console-address', '127.0.0.1:9001') -WorkingDirectory $root -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $tools 'native-minio-out.log') -RedirectStandardError (Join-Path $tools 'native-minio-error.log')
  $records += @{ id = $minioProcess.Id; executable = $minio; marker = $minioData; started = $minioProcess.StartTime.ToUniversalTime().ToString('o') }
  @{ data = $data; pgBin = $pgBin; processes = $records } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $stateFile -Encoding utf8
  $storageReady = $false
  for ($attempt=0; $attempt -lt 60; $attempt++) {
    try { $null = Invoke-WebRequest 'http://127.0.0.1:9000/minio/health/live' -UseBasicParsing -TimeoutSec 2; $storageReady = $true; break } catch { Start-Sleep -Milliseconds 500 }
  }
  if (-not $storageReady) { throw 'MinIO nao ficou pronto; consulte .tools/native-minio-error.log.' }
  & $node $storageSetup > (Join-Path $tools 'native-storage-setup.log') 2>&1
  if ($LASTEXITCODE -ne 0) { throw 'Falha ao preparar bucket/CORS; consulte .tools/native-storage-setup.log.' }
  $apiProcess = Start-Process -FilePath $api -WorkingDirectory $root -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $tools 'native-api-out.log') -RedirectStandardError (Join-Path $tools 'native-api-error.log')
  $records += @{ id = $apiProcess.Id; executable = $api; marker = ''; started = $apiProcess.StartTime.ToUniversalTime().ToString('o') }
  @{ data = $data; pgBin = $pgBin; processes = $records } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $stateFile -Encoding utf8
  $ready = $false
  for ($attempt=0; $attempt -lt 60; $attempt++) {
    try { $null = Invoke-RestMethod 'http://127.0.0.1:8080/health' -TimeoutSec 2; $ready = $true; break } catch { Start-Sleep -Milliseconds 500 }
  }
  if (-not $ready) { throw 'API nao ficou pronta. Consulte .tools/native-api-error.log e execute stop-native.ps1.' }
  $webProcess = Start-Process -FilePath $node -ArgumentList @(('"' + $vite + '"'), '--host', '127.0.0.1', '--port', '5173', '--strictPort') -WorkingDirectory (Join-Path $root 'apps\web') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $tools 'native-web-out.log') -RedirectStandardError (Join-Path $tools 'native-web-error.log')
  $records += @{ id = $webProcess.Id; executable = $node; marker = $vite; started = $webProcess.StartTime.ToUniversalTime().ToString('o') }
  @{ data = $data; pgBin = $pgBin; processes = $records } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $stateFile -Encoding utf8
  Start-Sleep -Seconds 2
  if ($webProcess.HasExited) { throw 'Frontend nao iniciou. Consulte .tools/native-web-error.log.' }
  Write-Output 'RoomForge nativo: http://localhost:5173 | API: http://127.0.0.1:8080/health'
} catch {
  if (Test-Path -LiteralPath $stateFile) { & (Join-Path $PSScriptRoot 'stop-native.ps1') }
  elseif ($postgresStarted -and -not (Test-Path -LiteralPath $stateFile)) {
    $pgControl = Start-Process -FilePath (Join-Path $pgBin 'pg_ctl.exe') -ArgumentList @('-D', ('"' + $data + '"'), '-m', 'fast', '-t', '60', '-w', 'stop') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $tools 'native-rollback.log') -RedirectStandardError (Join-Path $tools 'native-rollback-error.log')
    $null = $pgControl.Handle
  $pgDeadline = [DateTime]::UtcNow.AddSeconds(60)
  while (-not $pgControl.WaitForExit(1000)) {
    if ([DateTime]::UtcNow -ge $pgDeadline) { throw 'Tempo limite de pg_ctl excedido; consulte logs PostgreSQL.' }
  }
  $pgControl.Refresh()
    if ($null -eq $pgControl.ExitCode -or $pgControl.ExitCode -ne 0) { Write-Warning 'Falha no rollback PostgreSQL; consulte .tools/native-rollback-error.log.' }
  }
  throw
} finally {
  foreach ($key in $oldEnvironment.Keys) { [Environment]::SetEnvironmentVariable($key, $oldEnvironment[$key], 'Process') }
}












