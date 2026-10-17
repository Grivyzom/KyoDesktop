@echo off
setlocal
REM Rebuild script wrapper for Kyo (Frontend + Backend)
REM Usage:
REM   rebuild.bat
REM   rebuild.bat -Debug
REM   rebuild.bat -FrontendOnly
REM   rebuild.bat -BackendOnly
REM   rebuild.bat -Run

set SCRIPT_DIR=%~dp0
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\rebuild.ps1" %*
if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] La reconstruccion fallo con codigo de salida %ERRORLEVEL%.
    exit /b %ERRORLEVEL%
)
endlocal
