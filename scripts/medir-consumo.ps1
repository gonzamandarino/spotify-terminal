<#
.SYNOPSIS
  Mide RAM (working set) y CPU de spotify-terminal durante N minutos.

.DESCRIPTION
  Espera a que el proceso aparezca, lo muestrea cada -Intervalo segundos y
  al final imprime promedio y maximo de RAM y CPU. El CPU se expresa como
  porcentaje del total de la maquina (como el Administrador de tareas).
  Sirve para los AC-5/AC-6 del spec 001.

.EXAMPLE
  # Terminal 1:  cargo run --release -- play <album o playlist>
  # Terminal 2:
  .\scripts\medir-consumo.ps1 -Minutos 30
#>
param(
    [int]$Minutos = 30,
    [int]$Intervalo = 5,
    [string]$Proceso = "spotify-terminal",
    [string]$Csv = ""
)

$ErrorActionPreference = "Stop"
$nucleos = [Environment]::ProcessorCount

Write-Host "Esperando el proceso '$Proceso'..."
$p = $null
while (-not $p) {
    $p = Get-Process -Name $Proceso -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $p) { Start-Sleep -Seconds 1 }
}
Write-Host "Midiendo PID $($p.Id) durante $Minutos min (cada $Intervalo s)..."

$muestras = @()
$fin = (Get-Date).AddMinutes($Minutos)
$cpuAntes = $p.TotalProcessorTime
$horaAntes = Get-Date

while ((Get-Date) -lt $fin) {
    Start-Sleep -Seconds $Intervalo
    $p.Refresh()
    if ($p.HasExited) {
        Write-Warning "El proceso termino antes de tiempo."
        break
    }
    $ahora = Get-Date
    $cpuAhora = $p.TotalProcessorTime
    $cpu = ($cpuAhora - $cpuAntes).TotalSeconds / ($ahora - $horaAntes).TotalSeconds / $nucleos * 100
    $muestra = [pscustomobject]@{
        Hora     = $ahora.ToString("HH:mm:ss")
        RamMB    = [math]::Round($p.WorkingSet64 / 1MB, 1)
        PrivMB   = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
        CpuPct   = [math]::Round($cpu, 2)
    }
    $muestras += $muestra
    Write-Host ("{0}  RAM {1,6} MB  privada {2,6} MB  CPU {3,5} %" -f $muestra.Hora, $muestra.RamMB, $muestra.PrivMB, $muestra.CpuPct)
    $cpuAntes = $cpuAhora
    $horaAntes = $ahora
}

if ($muestras.Count -eq 0) {
    Write-Error "Sin muestras."
}

if ($Csv) {
    $muestras | Export-Csv -Path $Csv -NoTypeInformation -Encoding utf8
}

$ram = $muestras | Measure-Object -Property RamMB -Average -Maximum
$priv = $muestras | Measure-Object -Property PrivMB -Average -Maximum
$cpuStats = $muestras | Measure-Object -Property CpuPct -Average -Maximum
Write-Host ""
Write-Host "Resumen ($($muestras.Count) muestras, $nucleos nucleos logicos):"
Write-Host ("  RAM working set: promedio {0:N1} MB, maximo {1:N1} MB" -f $ram.Average, $ram.Maximum)
Write-Host ("  RAM privada:     promedio {0:N1} MB, maximo {1:N1} MB" -f $priv.Average, $priv.Maximum)
Write-Host ("  CPU:             promedio {0:N2} %,  maximo {1:N2} %" -f $cpuStats.Average, $cpuStats.Maximum)
