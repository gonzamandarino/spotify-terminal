<#
.SYNOPSIS
  Crea accesos directos a la app de escritorio (spotify-desktop.exe) en el
  escritorio y en el menu Inicio.

.DESCRIPTION
  Dos modos (spec 008):
  - Desde la carpeta del zip (spotify-desktop.exe esta al lado del script):
    usa ese .exe y arranca en esa carpeta.
  - Desde el repo (scripts\): usa target\release\spotify-desktop.exe y
    arranca en la carpeta del repo, para que la app encuentre el .env.
  Sin .env, la app toma el Client ID de %APPDATA%\spotify-terminal, asi que
  la carpeta de inicio no importa. Usa el icono del .exe. Volver a correrlo
  reemplaza los accesos directos.

.EXAMPLE
  # Desde la carpeta del zip (o doble clic en crear-accesos-directos.cmd):
  .\instalar-acceso-directo.ps1

.EXAMPLE
  # Desde el repo:
  cargo build --release
  .\scripts\instalar-acceso-directo.ps1

.EXAMPLE
  # Solo en una carpeta (p. ej. para probar):
  .\scripts\instalar-acceso-directo.ps1 -Destinos C:\temp
#>
param(
    [string]$Exe = "",
    [string[]]$Destinos = @(
        [Environment]::GetFolderPath("Desktop"),
        (Join-Path ([Environment]::GetFolderPath("Programs")) "")
    ),
    [string]$Nombre = "spotify-terminal"
)

$ErrorActionPreference = "Stop"
$junto = Join-Path $PSScriptRoot "spotify-desktop.exe"
if (Test-Path $junto) {
    # Carpeta del zip.
    $inicio = $PSScriptRoot
    if ($Exe -eq "") { $Exe = $junto }
} else {
    # Repo: el script esta en scripts\.
    $inicio = Split-Path -Parent $PSScriptRoot
    if ($Exe -eq "") { $Exe = Join-Path $inicio "target\release\spotify-desktop.exe" }
}
if (-not (Test-Path $Exe)) {
    throw "No existe $Exe. Compila primero con: cargo build --release"
}
$Exe = (Resolve-Path $Exe).Path

$shell = New-Object -ComObject WScript.Shell
foreach ($destino in $Destinos) {
    if (-not (Test-Path $destino)) {
        New-Item -ItemType Directory -Path $destino | Out-Null
    }
    $ruta = Join-Path $destino "$Nombre.lnk"
    $acceso = $shell.CreateShortcut($ruta)
    $acceso.TargetPath = $Exe
    $acceso.WorkingDirectory = $inicio
    $acceso.IconLocation = "$Exe,0"
    $acceso.Description = "Cliente de Spotify liviano"
    $acceso.Save()
    Write-Host "Acceso directo: $ruta"
}
