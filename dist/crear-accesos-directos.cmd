@echo off
rem Crea accesos directos a spotify-desktop.exe en el escritorio y en el menu
rem Inicio. Llama al .ps1 con la politica de ejecucion abierta solo para este
rem proceso: un .ps1 bajado de internet no corre con doble clic (spec 008).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0instalar-acceso-directo.ps1"
pause
