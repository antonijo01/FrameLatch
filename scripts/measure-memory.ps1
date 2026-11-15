param([ValidateSet('probe','record','record-cycles','idle')][string]$Mode = 'probe', [int]$Monitor = 0, [int]$Seconds = 10, [int]$Cycles = 3, [string]$Output = '')
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot '../target/release/framelatch.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw 'Build the release executable first.' }
switch ($Mode) {
 'probe' { $arguments = @('probe', $Monitor, $Seconds, $Cycles) }
 'idle' { $arguments = @('idle', $Seconds) }
 'record-cycles' {
   if (-not $Output -or -not [System.IO.Path]::IsPathRooted($Output)) { throw 'Output folder must be an absolute path.' }
   $arguments = @('record-cycles', $Monitor, $Seconds, ('"' + $Output + '"'), $Cycles)
 }
 'record' {
   if (-not $Output) { throw 'Pass -Output with a new absolute path to an MP4 file.' }
   if (-not [System.IO.Path]::IsPathRooted($Output)) { throw 'Output path must be absolute.' }
   $arguments = @('record', $Monitor, $Seconds, ('"' + $Output + '"'))
 }
}
$process = Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru
try {
 $samples = while (-not $process.HasExited) {
  $childIds = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($process.Id)" | Select-Object -ExpandProperty ProcessId)
  $ids = @($process.Id) + $childIds | Select-Object -Unique
  $live = @(foreach ($processId in $ids) { Get-Process -Id $processId -ErrorAction SilentlyContinue })
  $parent = @($live | Where-Object { $_.Id -eq $process.Id })
  if ($parent.Count -gt 0) {
    [pscustomobject]@{
      Time = Get-Date -Format o
      ProcessCount = $live.Count
      PrivateMiB = [math]::Round((($live | Measure-Object PrivateMemorySize64 -Sum).Sum) / 1MB, 2)
      WorkingSetMiB = [math]::Round((($live | Measure-Object WorkingSet64 -Sum).Sum) / 1MB, 2)
    }
  }
  Start-Sleep -Milliseconds 500
 }
 $process.WaitForExit()
 $samples | Format-Table -AutoSize
 if ($process.ExitCode -ne 0) { throw "Capture process exited with code $($process.ExitCode)" }
} finally { if (-not $process.HasExited) { Stop-Process -Id $process.Id } }
