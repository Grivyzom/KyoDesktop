$ErrorActionPreference = "Stop"

# Obtener el target triple de rustc
$targetTriple = (rustc -Vv | Select-String "host:" | ForEach-Object { $_.ToString().Split(" ")[1].Trim() })
$ext = if ($IsWindows -or $env:OS -match "Windows") { ".exe" } else { "" }
$outDir = Join-Path $PSScriptRoot "..\src-tauri\binaries"
$outFile = Join-Path $outDir "go-server-$targetTriple$ext"

if (!(Test-Path $outDir)) {
    New-Item -ItemType Directory -Path $outDir | Out-Null
}

Write-Host "Compilando Go Sidecar para $targetTriple..."
Push-Location (Join-Path $PSScriptRoot "..\sidecar")
go build -ldflags="-s -w" -o $outFile main.go
Pop-Location
Write-Host "Sidecar compilado en: $outFile"
