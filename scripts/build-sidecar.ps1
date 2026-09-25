# Script para compilar el Sidecar Go con la nomenclatura de target requerida por Tauri v2 en Windows
$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$sidecarDir = Join-Path $projectRoot "sidecar"
$binariesDir = Join-Path $projectRoot "src-tauri\binaries"

if (-not (Test-Path $binariesDir)) {
    New-Item -ItemType Directory -Path $binariesDir -Force | Out-Null
}

$outputExe = Join-Path $binariesDir "go-server-x86_64-pc-windows-msvc.exe"

Write-Host "==> Compilando Go Sidecar..." -ForegroundColor Cyan
Write-Host "    Fuente: $sidecarDir"
Write-Host "    Destino: $outputExe"

$env:GOOS = "windows"
$env:GOARCH = "amd64"
$env:CGO_ENABLED = "0"

Push-Location $sidecarDir
try {
    go build -ldflags "-s -w" -o $outputExe .
    Write-Host "==> ¡Compilación exitosa! Binario generado:" -ForegroundColor Green
    Get-Item $outputExe | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize
} finally {
    Pop-Location
}
