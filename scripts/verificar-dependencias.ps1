<#
.SYNOPSIS
  Falla si los .exe de release dependen del runtime de C/C++ (spec 008,
  AC-10).

.DESCRIPTION
  Lista las DLL de cada .exe con dumpbin /dependents (de las Build Tools
  de Visual Studio, se busca con vswhere) y falla si aparece vcruntime*,
  msvcp*, ucrtbase o api-ms-win-crt-*. Lo usa el workflow de release; se
  puede correr en local despues de cargo build --release.

.EXAMPLE
  cargo build --release
  .\scripts\verificar-dependencias.ps1
#>
param(
    [string]$Carpeta = (Join-Path (Split-Path -Parent $PSScriptRoot) "target\release")
)

$ErrorActionPreference = "Stop"
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$dumpbin = Get-ChildItem (Join-Path $vs "VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe") | Select-Object -First 1
if (-not $dumpbin) {
    throw "No se encontro dumpbin.exe (hacen falta las Build Tools de Visual Studio con C++)."
}

$fallo = $false
foreach ($nombre in "spotify-terminal.exe", "spotify-desktop.exe") {
    $exe = Join-Path $Carpeta $nombre
    if (-not (Test-Path $exe)) { throw "No existe $exe." }
    $dlls = & $dumpbin.FullName /nologo /dependents $exe |
        Where-Object { $_ -match '^\s+\S+\.dll\s*$' } |
        ForEach-Object { $_.Trim() }
    Write-Host "${nombre}: $($dlls -join ', ')"
    $crt = $dlls | Where-Object { $_ -match '^(vcruntime|msvcp|ucrtbase|api-ms-win-crt-)' }
    if ($crt) {
        Write-Host "  ERROR: depende del runtime de C/C++: $($crt -join ', ')"
        $fallo = $true
    }
}
if ($fallo) { exit 1 }
Write-Host "OK: ningun .exe depende del runtime de C/C++."
