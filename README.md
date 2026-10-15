# TechBench

A small PowerShell toolkit for basic Windows troubleshooting by IT support staff. No install, no dependencies beyond Windows PowerShell 5.1+.

## Current features

**Network diagnostics**

- Show active adapters with IP, gateway and DNS
- Connectivity ladder: gateway, internet IP, DNS resolution, HTTPS
- Flush DNS cache
- Test a TCP port on a host
- Trace route
- Reset Winsock/TCP-IP stack (requires admin and a reboot)

## Usage

```powershell
git clone https://github.com/<your-username>/techbench.git
cd techbench
powershell -ExecutionPolicy Bypass -File .\TechBench.ps1
```

Run as Administrator for the network stack reset.

## Roadmap

- [ ] System info report
- [ ] Disk checks and cleanup (SFC / DISM / chkdsk)
- [ ] Services and process troubleshooting
- [ ] Event log error summary
- [ ] Export results to a text/HTML report

## Safety

Every script that changes system state asks for confirmation first. Read the code before running it on production machines.

## License

MIT
