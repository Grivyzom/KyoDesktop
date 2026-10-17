<#
.SYNOPSIS
    Script integral de compilacion y reconstruccion para Kyo (KyoDesktop).
    Reconstruye el Frontend (Vue 3 / Vite) y todos los componentes del Backend
    (Go Sidecar, Dioxus Chat Desktop y Tauri Host App).

.DESCRIPTION
    Este script automatiza el ciclo completo de reconstruccion:
      1. Detiene procesos activos (Kyo-agent, desktop, go-server) para evitar
         errores de bloqueo de archivos en Windows (PermissionDenied / LNK1104).
      2. Reconstruye el Sidecar en Go (servidor HTTP local / replica).
      3. Reconstruye el Frontend web (Vue 3 + TypeScript + Tailwind con Vite).
      4. Reconstruye Dioxus Chat Desktop (Rust).
      5. Reconstruye el Host Tauri v2 (Rust -> Kyo-agent.exe).
      6. Sincroniza y coloca los binarios en las ubicaciones requeridas.
      7. Muestra un resumen visual de tiempos y estados de cada componente.

.PARAMETER Mode
    Modo de compilacion: 'release' (por defecto, optimizado) o 'debug' (rapido).
.PARAMETER Debug
    Modo rapido de depuracion (alias para -Mode debug).
.PARAMETER Release
    Modo optimizado de produccion (alias para -Mode release).
.PARAMETER FrontendOnly
    Reconstruye unicamente el Frontend (Vue 3 / Vite).
.PARAMETER BackendOnly
    Reconstruye todos los componentes del Backend (Sidecar Go, Dioxus Chat y Tauri Host).
.PARAMETER SidecarOnly
    Reconstruye unicamente el Sidecar en Go.
.PARAMETER DioxusOnly
    Reconstruye unicamente el Dioxus Chat en Rust.
.PARAMETER TauriOnly
    Reconstruye unicamente el Host principal de Tauri.
.PARAMETER Clean
    Limpia los artefactos de compilacion previos (dist/, target/, etc.) antes de compilar.
.PARAMETER Bundle
    Genera el paquete instalador completo de Tauri (MSI/NSIS).
.PARAMETER Run
    Ejecuta Kyo-agent automaticamente al finalizar exitosamente la reconstruccion.
.PARAMETER NoKill
    No intenta detener procesos en ejecucion antes de compilar.

.EXAMPLE
    .\scripts\rebuild.ps1
    Reconstruye todo (Frontend y Backend completo) en modo Release.

.EXAMPLE
    .\scripts\rebuild.ps1 -Debug
    Reconstruye todo en modo Debug para desarrollo rapido.

.EXAMPLE
    .\scripts\rebuild.ps1 -FrontendOnly
    Reconstruye unicamente el Frontend.

.EXAMPLE
    .\scripts\rebuild.ps1 -BackendOnly -Run
    Reconstruye el Backend y ejecuta Kyo inmediatamente.
#>

param (
    [Parameter(Position = 0)]
    [ValidateSet('release', 'debug', IgnoreCase = $true)]
    [string]$Mode = 'release',

    [switch]$Dev,
    [switch]$Release,
    [switch]$FrontendOnly,
    [switch]$BackendOnly,
    [switch]$SidecarOnly,
    [switch]$DioxusOnly,
    [switch]$TauriOnly,
    [switch]$Clean,
    [switch]$Bundle,
    [switch]$Run,
    [switch]$NoKill
)

$ErrorActionPreference = 'Stop'
$globalStart = [System.Diagnostics.Stopwatch]::StartNew()

# --- 0. Ajuste de Modo ---
if ($Dev -or ($Mode.ToLower() -eq 'debug')) {
    $Mode = 'debug'
} elseif ($Release) {
    $Mode = 'release'
}
$isRelease = ($Mode.ToLower() -eq 'release')
$buildTarget = if ($isRelease) { 'release' } else { 'debug' }

# --- 1. Resolucion de Rutas del Proyecto ---
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir

$frontendDir  = $projectRoot
$sidecarDir   = Join-Path $projectRoot "sidecar"
$dioxusDir    = Join-Path $projectRoot "dioxus-chat"
$tauriDir     = Join-Path $projectRoot "src-tauri"
$binariesDir  = Join-Path $tauriDir "binaries"
$distDir      = Join-Path $projectRoot "dist"

# Determinar que componentes construir
$buildFrontend = $true
$buildSidecar  = $true
$buildDioxus   = $true
$buildTauri    = $true

if ($FrontendOnly) {
    $buildFrontend = $true
    $buildSidecar  = $false
    $buildDioxus   = $false
    $buildTauri    = $false
} elseif ($BackendOnly) {
    $buildFrontend = $false
    $buildSidecar  = $true
    $buildDioxus   = $true
    $buildTauri    = $true
} elseif ($SidecarOnly) {
    $buildFrontend = $false
    $buildSidecar  = $true
    $buildDioxus   = $false
    $buildTauri    = $false
} elseif ($DioxusOnly) {
    $buildFrontend = $false
    $buildSidecar  = $false
    $buildDioxus   = $true
    $buildTauri    = $false
} elseif ($TauriOnly) {
    $buildFrontend = $false
    $buildSidecar  = $false
    $buildDioxus   = $false
    $buildTauri    = $true
}

# --- Formato de Mensajes ---
function Write-Header {
    param([string]$Title)
    Write-Host ""
    Write-Host ("=" * 70) -ForegroundColor Cyan
    Write-Host "  $Title" -ForegroundColor White
    Write-Host ("=" * 70) -ForegroundColor Cyan
}

function Write-Step {
    param([string]$Num, [string]$StepName)
    Write-Host ""
    Write-Host "[$Num] $StepName" -ForegroundColor Yellow
}

function Write-Success {
    param([string]$Msg)
    Write-Host "    [OK] $Msg" -ForegroundColor Green
}

function Write-Info {
    param([string]$Msg)
    Write-Host "    [i]  $Msg" -ForegroundColor Gray
}

function Write-WarnMsg {
    param([string]$Msg)
    Write-Host "    [!]  $Msg" -ForegroundColor DarkYellow
}

function Write-ErrMsg {
    param([string]$Msg)
    Write-Host "    [X]  $Msg" -ForegroundColor Red
}

function Format-Size {
    param([long]$Bytes)
    if ($Bytes -ge 1GB) { return "{0:N2} GB" -f ($Bytes / 1GB) }
    if ($Bytes -ge 1MB) { return "{0:N2} MB" -f ($Bytes / 1MB) }
    if ($Bytes -ge 1KB) { return "{0:N2} KB" -f ($Bytes / 1KB) }
    return "$Bytes B"
}

# --- Inicio ---
Write-Header "KYO APP REBUILD SCRIPT - Modo: $($Mode.ToUpper())"
Write-Info "Directorio base: $projectRoot"

$componentsToBuild = @()
if ($buildFrontend) { $componentsToBuild += "Frontend (Vue 3)" }
if ($buildSidecar)  { $componentsToBuild += "Go Sidecar" }
if ($buildDioxus)   { $componentsToBuild += "Dioxus Chat" }
if ($buildTauri)    { $componentsToBuild += "Tauri Host" }
Write-Info "Componentes seleccionados: $($componentsToBuild -join ', ')"

# --- 2. Detencion Segura de Procesos Activos (Evitar bloqueos en Windows) ---
if (-not $NoKill) {
    $targetProcessNames = @('Kyo-agent', 'desktop', 'go-server', 'tauri-app')
    $activeProcs = Get-Process -Name $targetProcessNames -ErrorAction SilentlyContinue

    if ($activeProcs) {
        Write-Step "0" "Deteniendo instancias previas de Kyo..."
        foreach ($proc in $activeProcs) {
            Write-WarnMsg "Deteniendo proceso $($proc.ProcessName) (PID: $($proc.Id))..."
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        }
        Start-Sleep -Milliseconds 600
        Write-Success "Procesos liberados correctamente."
    }
}

# --- 3. Verificacion de Herramientas Requeridas ---
function Assert-Tool {
    param([string]$Cmd, [string]$Name)
    $found = Get-Command $Cmd -ErrorAction SilentlyContinue
    if (-not $found) {
        Write-ErrMsg "No se encontro '$Cmd' en el PATH. Por favor instala $Name."
        throw "Herramienta faltante: $Cmd"
    }
}

Assert-Tool "cargo" "Rust (cargo)"
if ($buildFrontend -or $buildTauri) {
    $hasPnpm = Get-Command "pnpm" -ErrorAction SilentlyContinue
    $pmCmd = if ($hasPnpm) { "pnpm" } else { "npm" }
}
if ($buildSidecar) {
    Assert-Tool "go" "Go (golang)"
}

# --- 4. Limpieza Opcional (-Clean) ---
if ($Clean) {
    Write-Step "C" "Limpiando artefactos previos (-Clean)..."
    if ($buildFrontend -and (Test-Path $distDir)) {
        Remove-Item -Path $distDir -Recurse -Force -ErrorAction SilentlyContinue
        Write-Info "Carpeta dist/ eliminada."
    }
    if ($buildSidecar) {
        $sidecarBin = Join-Path $binariesDir "go-server-x86_64-pc-windows-msvc.exe"
        if (Test-Path $sidecarBin) {
            Remove-Item -Path $sidecarBin -Force -ErrorAction SilentlyContinue
            Write-Info "Binario de sidecar eliminado."
        }
    }
    Write-Success "Limpieza completada."
}

# Tabla de resultados para el resumen final
$results = [System.Collections.Generic.List[PSCustomObject]]::new()

# --- 5. Compilacion: Sidecar Go ---
if ($buildSidecar) {
    Write-Step "1/4" "Compilando Backend: Go Sidecar Server..."
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    if (-not (Test-Path $binariesDir)) {
        New-Item -ItemType Directory -Path $binariesDir -Force | Out-Null
    }

    $targetExe = Join-Path $binariesDir "go-server-x86_64-pc-windows-msvc.exe"
    
    $prevEnvGOOS = $env:GOOS
    $prevEnvGOARCH = $env:GOARCH
    $prevEnvCGO = $env:CGO_ENABLED

    $env:GOOS = "windows"
    $env:GOARCH = "amd64"
    $env:CGO_ENABLED = "0"

    Push-Location $sidecarDir
    try {
        $goBuildArgs = @("build")
        if ($isRelease) {
            $goBuildArgs += @("-ldflags", "-s -w")
        }
        $goBuildArgs += @("-o", $targetExe, ".")

        Write-Info "go $($goBuildArgs -join ' ')"
        & go @goBuildArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Error al compilar Go Sidecar (codigo de salida: $LASTEXITCODE)"
        }

        # Sincronizar copias auxiliares para compatibilidad de rutas locales
        $sidecarCopies = @(
            (Join-Path $sidecarDir "go-server.exe"),
            (Join-Path $dioxusDir "go-server.exe")
        )
        $tauriTargetDir = Join-Path $tauriDir "target\$buildTarget"
        if (Test-Path $tauriTargetDir) {
            $sidecarCopies += (Join-Path $tauriTargetDir "go-server.exe")
            $sidecarCopies += (Join-Path $tauriTargetDir "go-server-x86_64-pc-windows-msvc.exe")
        }

        foreach ($copyPath in $sidecarCopies) {
            try {
                Copy-Item -Path $targetExe -Destination $copyPath -Force -ErrorAction SilentlyContinue
            } catch {
                # Ignorar si algun archivo temporal no se puede copiar
            }
        }

        $sw.Stop()
        $fi = Get-Item $targetExe
        $sizeStr = Format-Size $fi.Length
        Write-Success "Sidecar compilado exitosamente: $targetExe ($sizeStr en $($sw.Elapsed.TotalSeconds.ToString('F2'))s)"
        $results.Add([PSCustomObject]@{
            Component = "Go Sidecar"
            Status    = "SUCCESS"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "src-tauri/binaries/go-server-..."
        })
    } catch {
        $sw.Stop()
        Write-ErrMsg "Fallo al compilar Go Sidecar: $_"
        $results.Add([PSCustomObject]@{
            Component = "Go Sidecar"
            Status    = "FAILED"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "Error: $_"
        })
        throw
    } finally {
        $env:GOOS = $prevEnvGOOS
        $env:GOARCH = $prevEnvGOARCH
        $env:CGO_ENABLED = $prevEnvCGO
        Pop-Location
    }
}

# --- 6. Compilacion: Frontend (Vue 3 + Vite) ---
if ($buildFrontend) {
    Write-Step "2/4" "Compilando Frontend: Vue 3 + TypeScript (Vite)..."
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    Push-Location $frontendDir
    try {
        Write-Info "$pmCmd run build"
        & $pmCmd run build
        if ($LASTEXITCODE -ne 0) {
            throw "Error al compilar el Frontend con $pmCmd (codigo de salida: $LASTEXITCODE)"
        }

        $indexHtml = Join-Path $distDir "index.html"
        if (-not (Test-Path $indexHtml)) {
            throw "El build termino pero no se encontro dist/index.html"
        }

        $sw.Stop()
        Write-Success "Frontend compilado exitosamente en dist/ ($($sw.Elapsed.TotalSeconds.ToString('F2'))s)"
        $results.Add([PSCustomObject]@{
            Component = "Frontend (Vue 3)"
            Status    = "SUCCESS"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "dist/"
        })
    } catch {
        $sw.Stop()
        Write-ErrMsg "Fallo al compilar el Frontend: $_"
        $results.Add([PSCustomObject]@{
            Component = "Frontend (Vue 3)"
            Status    = "FAILED"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "Error: $_"
        })
        throw
    } finally {
        Pop-Location
    }
}

# --- 7. Compilacion: Dioxus Chat Desktop (Rust) ---
if ($buildDioxus) {
    Write-Step "3/4" "Compilando Backend/UI: Dioxus Chat Desktop (Rust)..."
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    $dioxusCargoToml = Join-Path $dioxusDir "Cargo.toml"
    $dioxusArgs = @("build", "--manifest-path", $dioxusCargoToml, "--package", "desktop")
    if ($isRelease) {
        $dioxusArgs += "--release"
    }

    try {
        Write-Info "cargo $($dioxusArgs -join ' ')"
        & cargo @dioxusArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Error al compilar Dioxus Chat (codigo de salida: $LASTEXITCODE)"
        }

        $compiledDioxusExe = Join-Path $dioxusDir "target\$buildTarget\desktop.exe"
        if (-not (Test-Path $compiledDioxusExe)) {
            throw "No se encontro el ejecutable generado en $compiledDioxusExe"
        }

        # Sincronizar en la raiz de dioxus-chat y en src-tauri/target para acceso directo
        $rootDioxusExe = Join-Path $dioxusDir "desktop.exe"
        Copy-Item -Path $compiledDioxusExe -Destination $rootDioxusExe -Force
        
        $tauriTargetDir = Join-Path $tauriDir "target\$buildTarget"
        if (Test-Path $tauriTargetDir) {
            Copy-Item -Path $compiledDioxusExe -Destination (Join-Path $tauriTargetDir "desktop.exe") -Force -ErrorAction SilentlyContinue
        }

        $sw.Stop()
        $fi = Get-Item $compiledDioxusExe
        $sizeStr = Format-Size $fi.Length
        Write-Success "Dioxus Chat compilado exitosamente: desktop.exe ($sizeStr en $($sw.Elapsed.TotalSeconds.ToString('F2'))s)"
        $results.Add([PSCustomObject]@{
            Component = "Dioxus Chat Desktop"
            Status    = "SUCCESS"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "dioxus-chat/desktop.exe"
        })
    } catch {
        $sw.Stop()
        Write-ErrMsg "Fallo al compilar Dioxus Chat: $_"
        $results.Add([PSCustomObject]@{
            Component = "Dioxus Chat Desktop"
            Status    = "FAILED"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "Error: $_"
        })
        throw
    }
}

# --- 8. Compilacion: Tauri Host (Rust) ---
if ($buildTauri) {
    Write-Step "4/4" "Compilando Backend Principal: Tauri Host App (Kyo-agent)..."
    $sw = [System.Diagnostics.Stopwatch]::StartNew()

    Push-Location $frontendDir
    try {
        $tauriArgs = @("tauri", "build")
        if (-not $Bundle) {
            $tauriArgs += "--no-bundle"
        }
        if (-not $isRelease) {
            $tauriArgs += "--debug"
        }

        Write-Info "$pmCmd $($tauriArgs -join ' ')"
        & $pmCmd @tauriArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Error al compilar Tauri Host con $pmCmd (codigo de salida: $LASTEXITCODE)"
        }

        $kyoExe = Join-Path $tauriDir "target\$buildTarget\Kyo-agent.exe"
        if (-not (Test-Path $kyoExe)) {
            $kyoExeAlt = Join-Path $tauriDir "target\$buildTarget\tauri-app.exe"
            if (Test-Path $kyoExeAlt) {
                $kyoExe = $kyoExeAlt
            }
        }

        $sw.Stop()
        $sizeStr = if (Test-Path $kyoExe) { Format-Size (Get-Item $kyoExe).Length } else { "N/A" }
        Write-Success "Tauri Host compilado exitosamente ($sizeStr en $($sw.Elapsed.TotalSeconds.ToString('F2'))s)"
        $results.Add([PSCustomObject]@{
            Component = "Tauri Host (Kyo-agent)"
            Status    = "SUCCESS"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "src-tauri/target/$buildTarget/Kyo-agent.exe"
        })
    } catch {
        $sw.Stop()
        Write-ErrMsg "Fallo al compilar Tauri Host: $_"
        $results.Add([PSCustomObject]@{
            Component = "Tauri Host (Kyo-agent)"
            Status    = "FAILED"
            Duration  = "$($sw.Elapsed.TotalSeconds.ToString('F2'))s"
            Output    = "Error: $_"
        })
        throw
    } finally {
        Pop-Location
    }
}

# --- 9. Resumen Final ---
$globalStart.Stop()
Write-Host ""
Write-Host ("=" * 70) -ForegroundColor Cyan
Write-Host "                RESUMEN DE RECONSTRUCCION - KYO" -ForegroundColor White
Write-Host ("=" * 70) -ForegroundColor Cyan

foreach ($res in $results) {
    $color = if ($res.Status -eq 'SUCCESS') { 'Green' } else { 'Red' }
    $mark  = if ($res.Status -eq 'SUCCESS') { 'OK' } else { 'FAIL' }
    Write-Host ("  [{0,4}] {1,-22} : {2,-7} ({3}) -> {4}" -f $mark, $res.Component, $res.Status, $res.Duration, $res.Output) -ForegroundColor $color
}

Write-Host ("-" * 70) -ForegroundColor DarkGray
Write-Host ("  TIEMPO TOTAL TRANSCURRIDO: {0:F2} segundos" -f $globalStart.Elapsed.TotalSeconds) -ForegroundColor White
Write-Host ("=" * 70) -ForegroundColor Cyan

# --- 10. Lanzamiento Opcional (-Run) ---
if ($Run) {
    $exeToRun = Join-Path $tauriDir "target\$buildTarget\Kyo-agent.exe"
    if (-not (Test-Path $exeToRun)) {
        $exeToRun = Join-Path $dioxusDir "desktop.exe"
    }

    if (Test-Path $exeToRun) {
        Write-Host ""
        Write-Host "[>] Iniciando aplicacion: $exeToRun" -ForegroundColor Cyan
        Start-Process -FilePath $exeToRun
    } else {
        Write-WarnMsg "No se encontro el ejecutable principal para iniciar: $exeToRun"
    }
}
