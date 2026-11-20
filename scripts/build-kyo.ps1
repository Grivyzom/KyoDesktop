<#
.SYNOPSIS
    Script rapido para compilar y reconstruir especificamente Kyo-agent.exe.
    Automatiza la reconstruccion del Frontend (Vue 3 / Vite) y del Tauri Host App,
    sincronizando el ejecutable Kyo-agent.exe en la raiz del proyecto.

.PARAMETER Mode
    Modo de compilacion: 'release' (por defecto) o 'debug'.
.PARAMETER Dev
    Modo rapido debug (alias para -Mode debug).
.PARAMETER Run
    Ejecuta Kyo-agent.exe inmediatamente tras compilar con exito.
.PARAMETER Clean
    Limpia los artefactos previos de frontend y tauri antes de compilar.
#>
param(
    [ValidateSet('release', 'debug', IgnoreCase = $true)]
    [string]$Mode = 'release',
    [switch]$Dev,
    [switch]$Run,
    [switch]$Clean,
    [switch]$Bundle
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$rebuildScript = Join-Path $scriptDir "rebuild.ps1"

$params = @{
    KyoOnly = $true
    Mode = if ($Dev) { 'debug' } else { $Mode }
}
if ($Run) { $params['Run'] = $true }
if ($Clean) { $params['Clean'] = $true }
if ($Bundle) { $params['Bundle'] = $true }

& $rebuildScript @params
