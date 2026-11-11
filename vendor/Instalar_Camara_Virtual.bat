@echo off
setlocal EnableDelayedExpansion
title Instalador Driver Camara Virtual DirectShow

echo ========================================================
echo   Instalador del Driver DirectShow para Camara Virtual
echo ========================================================
echo.

:: 1. Comprobar si ya esta instalado en el sistema
reg query "HKLM\SOFTWARE\Classes\CLSID\{860BB310-5D01-11d0-BD3B-00A0C911CE86}\Instance\{5c2cd55c-92ad-4999-8666-912bd3e70010}" >nul 2>&1
if %errorlevel% equ 0 (
    echo   *************************************************
    echo   [OK] El Driver DirectShow YA ESTA INSTALADO!
    echo   Dispositivo: "Unity Video Capture"
    echo.
    echo   Windows y OBS Studio ya lo reconocen perfectamente.
    echo   No necesitas hacer nada mas.
    echo   *************************************************
    echo.
    echo Presiona cualquier tecla para cerrar...
    pause >nul
    exit /b 0
)

:: 2. Si ya viene con el parametro ELEVATED, no volver a solicitar elevacion (evita bucles)
if "%~1"=="ELEVATED" goto :do_install

:: 3. Comprobar si ya se tienen permisos de Administrador
fltmc >nul 2>&1
if %errorlevel% equ 0 goto :do_install

net session >nul 2>&1
if %errorlevel% equ 0 goto :do_install

:: 4. Solicitar permisos de Administrador UNA SOLA VEZ
echo [INFO] Solicitando permisos de Administrador en Windows...
echo (Por favor, haz clic en "Si" en la ventana de confirmacion)
echo.

powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%~f0' -ArgumentList 'ELEVATED' -Verb RunAs"
exit /b

:do_install
set "SCRIPT_DIR=%~dp0"
set "DLL64="

if exist "%SCRIPT_DIR%gui\vendor\UnityCaptureFilter64.dll" (
    set "DLL64=%SCRIPT_DIR%gui\vendor\UnityCaptureFilter64.dll"
) else if exist "%SCRIPT_DIR%vendor\UnityCaptureFilter64.dll" (
    set "DLL64=%SCRIPT_DIR%vendor\UnityCaptureFilter64.dll"
) else if exist "%SCRIPT_DIR%UnityCaptureFilter64.dll" (
    set "DLL64=%SCRIPT_DIR%UnityCaptureFilter64.dll"
)

if not defined DLL64 (
    echo [ERROR] No se encontro UnityCaptureFilter64.dll
    echo.
    echo Presiona cualquier tecla para salir...
    pause >nul
    exit /b 1
)

echo Registrando driver en Windows...
echo Archivo: "!DLL64!"
regsvr32 /s "!DLL64!"

reg query "HKLM\SOFTWARE\Classes\CLSID\{860BB310-5D01-11d0-BD3B-00A0C911CE86}\Instance\{5c2cd55c-92ad-4999-8666-912bd3e70010}" >nul 2>&1
if %errorlevel% equ 0 (
    echo.
    echo   *************************************************
    echo   [EXITO TOTAL] Driver DirectShow instalado!
    echo   Dispositivo: "Unity Video Capture"
    echo.
    echo   Ya puedes abrir OBS Studio, Zoom, Discord o Teams
    echo   y seleccionarlo directamente como Camara Web.
    echo   *************************************************
) else (
    echo.
    echo   [AVISO] Verificando en modo interactivo...
    regsvr32 "!DLL64!"
)

echo.
echo Presiona cualquier tecla para cerrar esta ventana...
pause >nul
