param([Parameter(Mandatory)][string]$Path, [int]$MinPointers = 1)
# Limited saved-report check. This does not prove original pointer IDs stayed
# continuous, isolate virtual device IDs, or validate MOVE/history transitions.
$ErrorActionPreference = 'Stop'
$rows = @(Get-Content -LiteralPath $Path | ForEach-Object { ConvertFrom-Json $_ })
$summary = @($rows | Where-Object event -eq 'summary')
$motion = @($rows | Where-Object event -eq 'motion')
if ($summary.Count -ne 1 -or $summary[0].reason -ne 'volume_down') {
    throw 'Expected one explicitly saved receiver session'
}
if ($summary[0].truncated -or $summary[0].activePointersAtStop -ne 0) {
    throw 'Receiver session is incomplete'
}
if ($motion.Count -ne $summary[0].motionEvents -or $motion.Count -eq 0) {
    throw 'Missing receiver motion events'
}
if ($summary[0].maxPointers -lt $MinPointers) {
    throw "Expected at least $MinPointers simultaneous pointers"
}
if ($summary[0].cancelEvents -ne 0 -or @($motion | Where-Object action -eq 3).Count -ne 0) {
    throw 'Touch continuity failed: receiver observed ACTION_CANCEL'
}
if ($motion[-1].action -ne 1) {
    throw 'Touch continuity failed: last gesture did not end with ACTION_UP'
}
Write-Output "PASS: $($motion.Count) events, max $($summary[0].maxPointers) pointers, no CANCEL, final UP"
