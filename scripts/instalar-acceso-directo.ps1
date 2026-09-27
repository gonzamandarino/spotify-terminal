<#
.SYNOPSIS
  Crea accesos directos a la app de escritorio (spotify-desktop.exe) en el
  escritorio y en el menu Inicio.

.DESCRIPTION
  El acceso directo arranca en la carpeta del repo, para que la app
  encuentre el .env con SPOTIFY_CLIENT_ID. Usa el icono del .exe.
  Sirve para el AC-14 del spec 004. Volver a correrlo reemplaza los
  accesos directos.

.EXAMPLE
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
$repo = Split-Path -Parent $PSScriptRoot
if ($Exe -eq "") {
    $Exe = Join-Path $repo "target\release\spotify-desktop.exe"
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
    $acceso.WorkingDirectory = $repo
    $acceso.IconLocation = "$Exe,0"
    $acceso.Description = "Cliente de Spotify liviano"
    $acceso.Save()
    Write-Host "Acceso directo: $ruta"
}
