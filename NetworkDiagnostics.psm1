#Requires -Version 5.1
<#
.SYNOPSIS
    Network diagnostics for TechBench.
#>

function Write-TBResult {
    param([string]$Label, [bool]$Ok, [string]$Detail = '')
    $tag = if ($Ok) { '[ OK ]' } else { '[FAIL]' }
    $color = if ($Ok) { 'Green' } else { 'Red' }
    Write-Host ("{0} {1} {2}" -f $tag, $Label, $Detail) -ForegroundColor $color
}

function Get-TBNetworkInfo {
    <# Shows active adapters with IP, gateway and DNS servers. #>
    Get-NetIPConfiguration | Where-Object { $_.NetAdapter.Status -eq 'Up' } | ForEach-Object {
        [pscustomobject]@{
            Adapter = $_.InterfaceAlias
            IPv4    = ($_.IPv4Address.IPAddress -join ', ')
            Gateway = ($_.IPv4DefaultGateway.NextHop -join ', ')
            DNS     = ($_.DNSServer.ServerAddresses -join ', ')
        }
    } | Format-Table -AutoSize
}

function Test-TBConnectivity {
    <# Walks the usual troubleshooting ladder: gateway -> internet IP -> DNS -> HTTPS. #>
    param(
        [string]$InternetIP = '1.1.1.1',
        [string]$DnsName    = 'www.microsoft.com'
    )

    $gw = (Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
           Sort-Object RouteMetric | Select-Object -First 1).NextHop
    if ($gw) {
        Write-TBResult 'Default gateway reachable' (Test-Connection $gw -Count 2 -Quiet) "($gw)"
    } else {
        Write-TBResult 'Default gateway found' $false '(no default route)'
    }

    Write-TBResult 'Internet reachable by IP' (Test-Connection $InternetIP -Count 2 -Quiet) "($InternetIP)"

    try {
        $null = Resolve-DnsName $DnsName -ErrorAction Stop
        Write-TBResult 'DNS resolution' $true "($DnsName)"
    } catch {
        Write-TBResult 'DNS resolution' $false "($DnsName)"
    }

    try {
        $r = Invoke-WebRequest "https://$DnsName" -UseBasicParsing -TimeoutSec 8 -Method Head
        Write-TBResult 'HTTPS request' $true "(HTTP $($r.StatusCode))"
    } catch {
        Write-TBResult 'HTTPS request' $false "($($_.Exception.Message))"
    }
}

function Clear-TBDnsCache {
    Clear-DnsClientCache
    Write-Host 'DNS client cache flushed.' -ForegroundColor Green
}

function Reset-TBNetworkStack {
    <# Needs admin. Resets Winsock and TCP/IP; a reboot is required afterwards. #>
    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
               ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) {
        Write-Host 'Run PowerShell as Administrator to reset the network stack.' -ForegroundColor Yellow
        return
    }
    $answer = Read-Host 'This resets Winsock and TCP/IP and needs a reboot. Continue? (y/N)'
    if ($answer -ne 'y') { return }
    netsh winsock reset
    netsh int ip reset
    Write-Host 'Done. Please restart the computer.' -ForegroundColor Yellow
}

function Test-TBPort {
    param(
        [Parameter(Mandatory)][string]$ComputerName,
        [Parameter(Mandatory)][int]$Port
    )
    $r = Test-NetConnection -ComputerName $ComputerName -Port $Port -WarningAction SilentlyContinue
    Write-TBResult "TCP ${ComputerName}:${Port}" $r.TcpTestSucceeded
}

function Get-TBRoute {
    param([Parameter(Mandatory)][string]$Target)
    Test-NetConnection -ComputerName $Target -TraceRoute -WarningAction SilentlyContinue |
        Select-Object -ExpandProperty TraceRoute
}

Export-ModuleMember -Function Get-TBNetworkInfo, Test-TBConnectivity, Clear-TBDnsCache,
                              Reset-TBNetworkStack, Test-TBPort, Get-TBRoute
