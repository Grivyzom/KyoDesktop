@echo off
setlocal
REM Rebuild script wrapper for Kyo (Frontend + Backend + Kyo-agent)
REM Usage:
REM   rebuild.bat                         (Reconstruye todo: Sidecar + Frontend + Dioxus + Kyo-agent)
REM   rebuild.bat -KyoOnly                (Reconstruye especificamente Kyo-agent.exe)
REM   rebuild.bat -KyoAgentOnly           (Alias para -KyoOnly)
REM   rebuild.bat -DioxusOnly             (Reconstruye unicamente desktop.exe)
REM   rebuild.bat -SidecarOnly            (Reconstruye unicamente go-server.exe)
REM   rebuild.bat -FrontendOnly           (Reconstruye unicamente el frontend en dist/)
REM   rebuild.bat -BackendOnly            (Reconstruye Sidecar, Dioxus y Kyo-agent)
REM   rebuild.bat -Debug                  (Modo rapido debug)
REM   rebuild.bat -Run                    (Ejecuta Kyo-agent.exe al finalizar)
REM   rebuild.bat -Clean                  (Limpia artefactos previos)

set SCRIPT_DIR=%~dp0
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\rebuild.ps1" %*
if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] La reconstruccion fallo con codigo de salida %ERRORLEVEL%.
    exit /b %ERRORLEVEL%
)
endlocal
