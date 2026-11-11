param(
    [Parameter(Mandatory)]
    [ValidateRange(1, 2147483647)]
    [int]$AppProcessId,

    [ValidateRange(1, 120)]
    [int]$DurationSeconds = 10,

    [ValidateRange(100, 5000)]
    [int]$IntervalMilliseconds = 500
)

$ErrorActionPreference = 'Stop'
$samples = [System.Collections.Generic.List[double]]::new()
$clock = [System.Diagnostics.Stopwatch]::StartNew()

do {
    $processes = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, WorkingSetSize
    $app = $processes | Where-Object ProcessId -EQ $AppProcessId | Select-Object -First 1
    if ($null -eq $app) {
        throw "App process $AppProcessId exited or was not found."
    }

    $treeIds = [System.Collections.Generic.HashSet[uint32]]::new()
    [void]$treeIds.Add($app.ProcessId)
    do {
        $previousCount = $treeIds.Count
        foreach ($process in $processes) {
            if ($treeIds.Contains($process.ParentProcessId)) {
                [void]$treeIds.Add($process.ProcessId)
            }
        }
    } until ($treeIds.Count -eq $previousCount)

    $members = @($processes | Where-Object { $treeIds.Contains($_.ProcessId) })
    $bytes = ($members | Measure-Object -Property WorkingSetSize -Sum).Sum
    $samples.Add($bytes / 1MB)
    if ($clock.Elapsed.TotalSeconds -lt $DurationSeconds) {
        Start-Sleep -Milliseconds $IntervalMilliseconds
    }
} while ($clock.Elapsed.TotalSeconds -lt $DurationSeconds)

$steadyStart = [int][Math]::Floor($samples.Count / 2)
$steady = @($samples[$steadyStart..($samples.Count - 1)] | Sort-Object)
$medianLow = [int][Math]::Floor(($steady.Count - 1) / 2)
$medianHigh = [int][Math]::Ceiling(($steady.Count - 1) / 2)
$median = ($steady[$medianLow] + $steady[$medianHigh]) / 2

[pscustomobject]@{
    App = $app.Name
    AppProcessId = $AppProcessId
    DurationSeconds = [Math]::Round($clock.Elapsed.TotalSeconds, 2)
    Samples = $samples.Count
    PeakWorkingSetMiB = [Math]::Round(($samples | Measure-Object -Maximum).Maximum, 2)
    SteadyWorkingSetMiB = [Math]::Round($median, 2)
    LastSampleProcesses = @(
        $members | Group-Object Name | Sort-Object Name | Select-Object Name, Count
    )
} | ConvertTo-Json -Depth 3
