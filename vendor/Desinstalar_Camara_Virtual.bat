@echo off
setlocal EnableDelayedExpansion
title Desinstalador Driver Camara Virtual DirectShow

:: 1. Si ya se paso el parametro de elevacion, no volver a elevar
if "%~1"=="ELEVATED" goto :do_uninstall

:: 2. Comprobar si ya se tienen permisos de Administrador
fltmc >nul 2>&1
if %errorlevel% equ 0 goto :do_uninstall

net session >nul 2>&1
if %errorlevel% equ 0 goto :do_uninstall

:: 3. Solicitar elevacion UNA SOLA VEZ
echo ========================================================
echo   Desinstalador del Driver DirectShow para Camara Virtual
echo ========================================================
echo.
echo [INFO] Solicitando permisos de Administrador...
powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%~f0' -ArgumentList 'ELEVATED' -Verb RunAs"
exit /b

:do_uninstall
echo ========================================================
echo   Desinstalador del Driver DirectShow para Camara Virtual
echo ========================================================
echo.

set "SCRIPT_DIR=%~dp0"
set "DLL64="
set "DLL32="

if exist "%SCRIPT_DIR%gui\vendor\UnityCaptureFilter64.dll" (
    set "DLL64=%SCRIPT_DIR%gui\vendor\UnityCaptureFilter64.dll"
    set "DLL32=%SCRIPT_DIR%gui\vendor\UnityCaptureFilter32.dll"
) else if exist "%SCRIPT_DIR%vendor\UnityCaptureFilter64.dll" (
    set "DLL64=%SCRIPT_DIR%vendor\UnityCaptureFilter64.dll"
    set "DLL32=%SCRIPT_DIR%vendor\UnityCaptureFilter32.dll"
) else if exist "%SCRIPT_DIR%UnityCaptureFilter64.dll" (
    set "DLL64=%SCRIPT_DIR%UnityCaptureFilter64.dll"
    set "DLL32=%SCRIPT_DIR%UnityCaptureFilter32.dll"
)

if defined DLL64 (
    echo Desregistrando componente de 64 bits...
    regsvr32 /u /s "!DLL64!"
)

if defined DLL32 (
    if exist "!DLL32!" (
        echo Desregistrando componente de 32 bits...
        regsvr32 /u /s "!DLL32!"
    )
)

echo [OK] Driver DirectShow desinstalado correctamente.
echo.
echo Presiona cualquier tecla para cerrar...
pause >nul
