# Run Topaz Gigapixel without crowding the GPU, which the owner's chat model and other viewers share.
# Dot-source this. One job at a time, below normal priority, and it stops when free video memory runs short.

$script:MinFreeVramGb = 6.0

function Get-VramGb {
    # Total from the display adapters' registry entries (WMI's AdapterRAM stops at 4 GB); used from the Windows GPU
    # performance counters. Both are for the adapter using the most memory.
    $class = "HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}"
    $sizes = Get-ChildItem $class -ErrorAction SilentlyContinue | ForEach-Object {
        (Get-ItemProperty $_.PSPath -Name "HardwareInformation.qwMemorySize" -ErrorAction SilentlyContinue)."HardwareInformation.qwMemorySize"
    } | Where-Object { $_ }
    $total = ($sizes | Measure-Object -Maximum).Maximum
    $used = ((Get-Counter '\GPU Adapter Memory(*)\Dedicated Usage' -ErrorAction SilentlyContinue).CounterSamples | Measure-Object CookedValue -Maximum).Maximum
    if (-not $total -or $null -eq $used) { return $null }
    [pscustomobject]@{ TotalGb = [math]::Round($total / 1GB, 1); UsedGb = [math]::Round($used / 1GB, 1); FreeGb = [math]::Round(($total - $used) / 1GB, 1) }
}

function Stop-Gigapixel([int]$Id) {
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$Id" -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $Id -Force -ErrorAction SilentlyContinue
}

# Returns an object: Started, Stopped (ran short of memory), Seconds, FreeBefore, LowestFree, HighestUsed.
function Invoke-GuardedGigapixel {
    param([string]$Exe, [string[]]$Arguments, [string]$Log)
    if (Get-Process gigapixel -ErrorAction SilentlyContinue) {
        Write-Output "Gigapixel is already running: one job at a time."
        return [pscustomobject]@{ Started = $false; Stopped = $false; Seconds = 0; FreeBefore = $null; LowestFree = $null; HighestUsed = $null }
    }
    $v = Get-VramGb
    if (-not $v) {
        Write-Output "Cannot read video memory, so not starting."
        return [pscustomobject]@{ Started = $false; Stopped = $false; Seconds = 0; FreeBefore = $null; LowestFree = $null; HighestUsed = $null }
    }
    if ($v.FreeGb -lt $script:MinFreeVramGb) {
        Write-Output "Only $($v.FreeGb) GB of video memory free, under $($script:MinFreeVramGb) GB: not starting."
        return [pscustomobject]@{ Started = $false; Stopped = $false; Seconds = 0; FreeBefore = $v.FreeGb; LowestFree = $v.FreeGb; HighestUsed = $v.UsedGb }
    }
    $free0 = $v.FreeGb; $low = $v.FreeGb; $high = $v.UsedGb; $stopped = $false
    $t = Get-Date
    $p = Start-Process -FilePath $Exe -ArgumentList $Arguments -RedirectStandardOutput $Log -RedirectStandardError "$Log.err" -PassThru -NoNewWindow
    while (-not $p.HasExited) {
        Get-Process gigapixel -ErrorAction SilentlyContinue | ForEach-Object { try { $_.PriorityClass = 'BelowNormal' } catch {} }
        $v = Get-VramGb
        if ($v) {
            if ($v.FreeGb -lt $low) { $low = $v.FreeGb }
            if ($v.UsedGb -gt $high) { $high = $v.UsedGb }
            if ($v.FreeGb -lt $script:MinFreeVramGb) {
                Write-Output "Free video memory fell to $($v.FreeGb) GB: stopping Gigapixel."
                Stop-Gigapixel $p.Id
                $stopped = $true
                break
            }
        }
        Start-Sleep -Milliseconds 500
    }
    [pscustomobject]@{ Started = $true; Stopped = $stopped; Seconds = [math]::Round(((Get-Date) - $t).TotalSeconds, 1); FreeBefore = $free0; LowestFree = $low; HighestUsed = $high }
}
