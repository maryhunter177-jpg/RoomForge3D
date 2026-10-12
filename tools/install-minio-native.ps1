param([string]$GoVersion = 'go1.27.2', [string]$ArchivePath, [ValidateRange(1,2)][int]$BuildJobs = 1)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root '.tools'
$minioTag = 'RELEASE.2025-10-15T17-29-55Z'
$variables = [ordered]@{
  TEMP = (Join-Path $tools 'tmp')
  TMP = (Join-Path $tools 'tmp')
  TMPDIR = (Join-Path $tools 'tmp')
  GOTMPDIR = (Join-Path $tools 'tmp')
  GOTELEMETRYDIR = (Join-Path $tools 'go-telemetry')
  GOENV = 'off'
  GOROOT = (Join-Path $tools 'go')
  GOPATH = (Join-Path $tools 'go-work')
  GOCACHE = (Join-Path $tools 'go-cache')
  GOMODCACHE = (Join-Path $tools 'go-work\pkg\mod')
  GOBIN = (Join-Path $tools 'minio')
  CGO_ENABLED = '0'
  GOOS = 'windows'
  GOARCH = 'amd64'
  GOTOOLCHAIN = 'local'
  GOPROXY = 'https://proxy.golang.org,direct'
  GOSUMDB = 'sum.golang.org'
}
$previous = @{}
try {
  foreach ($key in $variables.Keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process'); [Environment]::SetEnvironmentVariable($key, $variables[$key], 'Process') }
  New-Item -ItemType Directory -Force -Path $tools,$env:GOBIN,$env:GOTMPDIR,$env:GOTELEMETRYDIR | Out-Null
  $metadata = Invoke-RestMethod 'https://go.dev/dl/?mode=json'
  $release = $metadata | Where-Object { $_.version -eq $GoVersion } | Select-Object -First 1
  if (-not $release) { $release = Invoke-RestMethod 'https://go.dev/dl/?mode=json&include=all' | Where-Object { $_.version -eq $GoVersion } | Select-Object -First 1 }
  if (-not $release) { throw 'Versao Go oficial nao encontrada.' }
  $archive = $release.files | Where-Object { $_.os -eq 'windows' -and $_.arch -eq 'amd64' -and $_.kind -eq 'archive' } | Select-Object -First 1
  if (-not $archive) { throw 'Archive Go Windows amd64 nao encontrado.' }
  if (-not $ArchivePath) { $ArchivePath = Join-Path $tools 'go-native.zip' }
  $resolvedArchive = [IO.Path]::GetFullPath($ArchivePath)
  if (-not $resolvedArchive.StartsWith([IO.Path]::GetFullPath($tools) + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Archive deve estar dentro de .tools.' }
  if (Test-Path -LiteralPath $resolvedArchive) {
    if ((Get-Item -LiteralPath $resolvedArchive).Length -ne $archive.size) { throw 'Archive existente incompleto ou de outra versao; nenhum download duplicado iniciado.' }
  } else {
    $partial = $resolvedArchive + '.partial'
    if (Test-Path -LiteralPath $partial) { throw 'Download parcial existente; investigue antes de continuar.' }
    & (Join-Path $env:SystemRoot 'System32\curl.exe') --fail --location --output $partial ('https://go.dev/dl/' + $archive.filename)
    if ($LASTEXITCODE -ne 0) { throw 'Download Go falhou; arquivo parcial preservado.' }
    Move-Item -LiteralPath $partial -Destination $resolvedArchive
  }
  $actual = (Get-FileHash -LiteralPath $resolvedArchive -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -ne $archive.sha256) { throw 'SHA256 Go divergente; compilacao cancelada.' }
  $go = Join-Path $tools 'go\bin\go.exe'
  if (-not (Test-Path -LiteralPath $go)) {
    $resolvedTools = [IO.Path]::GetFullPath($tools)
    if ($resolvedTools -ne [IO.Path]::GetFullPath((Join-Path $root '.tools'))) { throw 'Destino de extracao invalido.' }
    & (Join-Path $env:SystemRoot 'System32\tar.exe') -xf $resolvedArchive -C $resolvedTools
    if ($LASTEXITCODE -ne 0) { throw 'Extracao Go falhou.' }
  }
  $installedVersion = & $go version
  if ($LASTEXITCODE -ne 0 -or -not $installedVersion.Contains($release.version + ' ')) { throw 'Go local diverge do archive validado.' }
  Write-Output ('Go archive SHA256 validado; compilando MinIO com BuildJobs=' + $BuildJobs)
  & $go install -p $BuildJobs ('github.com/minio/minio@' + $minioTag)
  if ($LASTEXITCODE -ne 0) { throw 'Compilacao MinIO falhou.' }
  $binary = Join-Path $env:GOBIN 'minio.exe'
  if (-not (Test-Path -LiteralPath $binary)) { throw 'Compilacao nao produziu minio.exe.' }
  $manifest = @{ goVersion = $release.version; goArchive = $archive.filename; goArchiveSHA256 = $actual; minioTag = $minioTag; binarySHA256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant() }
  $manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $tools 'minio-install.json') -Encoding utf8
  Write-Output ('MinIO nativo compilado: ' + $binary)
} finally { foreach ($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') } }
