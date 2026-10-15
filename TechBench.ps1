#Requires -Version 5.1
<#
.SYNOPSIS
    TechBench - basic Windows troubleshooting toolkit for IT support.
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\TechBench.ps1
#>

$ErrorActionPreference = 'Continue'
Import-Module (Join-Path $PSScriptRoot 'NetworkDiagnostics.psm1') -Force

function Show-Menu {
    Clear-Host
    Write-Host '=== TechBench ===' -ForegroundColor Cyan
    Write-Host ' Network'
    Write-Host '  1) Show network info'
    Write-Host '  2) Run connectivity check'
    Write-Host '  3) Flush DNS cache'
    Write-Host '  4) Test a TCP port'
    Write-Host '  5) Trace route'
    Write-Host '  6) Reset network stack (admin, reboot needed)'
    Write-Host '  Q) Quit'
}

do {
    Show-Menu
    $choice = (Read-Host "`nSelect").Trim().ToUpper()
    switch ($choice) {
        '1' { Get-TBNetworkInfo }
        '2' { Test-TBConnectivity }
        '3' { Clear-TBDnsCache }
        '4' { Test-TBPort -ComputerName (Read-Host 'Host') -Port ([int](Read-Host 'Port')) }
        '5' { Get-TBRoute -Target (Read-Host 'Host') }
        '6' { Reset-TBNetworkStack }
        'Q' { break }
        default { Write-Host 'Invalid choice.' -ForegroundColor Yellow }
    }
    if ($choice -ne 'Q') { Read-Host "`nPress Enter to continue" | Out-Null }
} while ($choice -ne 'Q')
