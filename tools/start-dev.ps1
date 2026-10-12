$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
if (Test-Path -LiteralPath 'C:\Program Files\Docker\Docker\resources\bin\docker.exe') {
    $env:PATH = 'C:\Program Files\Docker\Docker\resources\bin;' + $env:PATH
}
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw 'Instale e abra o Docker Desktop com containers Linux antes de executar este script.'
}
if (-not (Test-Path -LiteralPath '.env')) {
    function New-LocalSecret {
        $bytes = New-Object byte[] 32
        $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
        try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
        return ([BitConverter]::ToString($bytes)).Replace('-', '').ToLowerInvariant()
    }
    $databasePassword = New-LocalSecret
    $jwtSecret = New-LocalSecret
    $storagePassword = New-LocalSecret
    $contents = Get-Content -LiteralPath '.env.example' -Raw
    $contents = $contents.Replace('replace-with-a-unique-local-database-password', $databasePassword)
    $contents = $contents.Replace('replace-with-a-random-secret-of-at-least-32-characters', $jwtSecret)
    $contents = $contents.Replace('replace-with-a-unique-local-storage-password', $storagePassword)
    [System.IO.File]::WriteAllText((Join-Path (Get-Location) '.env'), $contents)
    Write-Host 'Configuração local criada com segredos aleatórios.'
}
docker compose up --build --detach --wait
if ($LASTEXITCODE -ne 0) { throw 'Não foi possível iniciar os serviços. Execute docker compose logs para diagnosticar.' }
Write-Host 'RoomForge disponível em http://localhost:5173'
