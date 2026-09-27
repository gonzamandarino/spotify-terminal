<#
.SYNOPSIS
  Arma el zip para distribuir (spec 008) a partir de target\release.

.DESCRIPTION
  El zip se arma desde una lista explicita de archivos, nunca desde una
  carpeta entera: asi no puede meterse un .env ni un token. Lo usa el
  workflow de release; se puede correr en local para revisar el contenido.

.EXAMPLE
  cargo build --release
  .\scripts\armar-zip.ps1 -Version v0.1.0
#>
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Salida = ""
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
if ($Salida -eq "") { $Salida = Join-Path $repo "target\dist" }

# Archivo en el repo -> nombre dentro del zip.
$archivos = [ordered]@{
    "target\release\spotify-desktop.exe"   = "spotify-desktop.exe"
    "target\release\spotify-terminal.exe"  = "spotify-terminal.exe"
    "dist\LEEME.txt"                       = "LEEME.txt"
    "dist\crear-accesos-directos.cmd"      = "crear-accesos-directos.cmd"
    "scripts\instalar-acceso-directo.ps1"  = "instalar-acceso-directo.ps1"
}

$nombre = "spotify-terminal-$Version-windows-x64"
$staging = Join-Path $Salida $nombre
if (Test-Path $staging) { Remove-Item -Recurse -Force $staging }
New-Item -ItemType Directory -Path $staging | Out-Null
foreach ($origen in $archivos.Keys) {
    $ruta = Join-Path $repo $origen
    if (-not (Test-Path $ruta)) { throw "Falta $origen." }
    Copy-Item $ruta (Join-Path $staging $archivos[$origen])
}

$zip = Join-Path $Salida "$nombre.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $staging "*") -DestinationPath $zip
Write-Host "Zip: $zip"
Get-ChildItem $staging | ForEach-Object { Write-Host ("  {0,-30} {1,10:N0} bytes" -f $_.Name, $_.Length) }
if ($env:GITHUB_OUTPUT) { "zip=$zip" | Out-File -Append -Encoding utf8 $env:GITHUB_OUTPUT }
