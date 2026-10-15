# Techbench

An all-in-one toolbox for IT support technicians on Windows. Eight diagnostic tools in one window, all running on your own PC, with results you can copy as plain text straight into a ticket.

> Status: early (v0.1). Windows 10/11 only. The installer is unsigned.

## What it does today

| Tool | What it does |
| --- | --- |
| **Ping** | Sends ICMP echoes to a host or IP: count, size, timeout and Don't Fragment are adjustable, with per-echo status and a summary. **Continuous Ping** runs against up to three targets side by side with a live graph, loss, min/avg/max and jitter. |
| **Traceroute** | Lists each router hop to a target with three round trips per hop, hostnames filled in as they resolve, over IPv4 or IPv6. |
| **DNS Lookup** | Queries A, AAAA, CNAME, MX, TXT, NS, PTR and SOA records against your PC's DNS server, 8.8.8.8, 1.1.1.1 or one you type, and tells NXDOMAIN, SERVFAIL, empty answers and no response apart. |
| **Port Check** | Tests TCP ports on one target (lists and ranges, built-in presets such as Windows, Web, Mail, Database) and reports Open, Closed or Filtered with connect times. |
| **LAN Scan** | Finds devices in an address range using ARP and ping, with hostname, MAC and vendor (from a bundled offline IEEE OUI list). Larger ranges ask for confirmation. |
| **IP Info** | Shows each network adapter (addresses, gateway, DNS, MAC, DHCP lease) and your public IP, ISP and city. |
| **Speed Test** | Measures latency, jitter, download and upload, latency under load with a Low/Moderate/High verdict, and star ratings for web browsing, streaming, gaming and video calls. Uses Cloudflare's public speed endpoints, or Ookla's `speedtest.exe` if you install it yourself. Optional ISP plan speeds show "% of plan". |
| **Subnet Calculator** | Network, broadcast, masks, usable range and host count for IPv4 and IPv6, plus a check whether an address is in the subnet. |

Also: **Saved Targets** (labelled addresses you can pick in any tool, with JSON import and export), **action menus** to jump from a device or hop into another tool, **Copy as text** on every result, and a dark theme by default with a light toggle.

Everything works without Administrator rights. Only IP Info's public lookup and Speed Test use the internet.

## Install (Windows)

1. Download or build `Techbench_<version>_x64-setup.exe` (see below).
2. Run it. Windows SmartScreen warns about the unsigned installer the first time: choose **More info, then Run anyway**.
3. It installs for the current user only (`%LOCALAPPDATA%\Techbench`) and adds a Start menu entry. No UAC prompt.

To uninstall: **Settings, Apps, Installed apps, Techbench, Uninstall**.

### Requirements to run

- Windows 10 or 11 (64-bit)
- WebView2 runtime (included in Windows 11; the installer fetches it on Windows 10 if missing)
- Optional, for the Ookla option in Speed Test: [Ookla's Speedtest CLI](https://www.speedtest.net/apps/cli), installed and its licence accepted by you (`winget install Ookla.Speedtest.CLI`, then run `speedtest` once). Techbench does not bundle it.

## Build and run from source

Requirements:

- Windows 10 or 11
- [Node.js](https://nodejs.org/) 20 or later
- [Rust](https://rustup.rs/) with the `x86_64-pc-windows-msvc` toolchain
- Visual Studio Build Tools 2022 with the **Desktop development with C++** workload:

  ```powershell
  winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  ```

```powershell
npm install
npm run tauri dev      # run with hot reload
npm test               # all Rust tests
npm run package        # build the installer: target\release\bundle\nsis\
```

`cargo test --workspace -- --ignored` also runs the opt-in tests that reach the internet and move real data.

## Things to know

- **What it changes on your PC:** nothing system-wide. It does not touch services, the registry or network settings. It writes your Saved Targets to the app's data folder, and an export file only where you choose.
- **Network traffic it sends:** pings, ARP, DNS and TCP connection tests, and speed tests. Only scan networks you are allowed to scan.
- **Speed Test and Cloudflare:** Cloudflare publishes no terms for third-party use of its speed endpoints, and its `robots.txt` disallows them. Techbench keeps requests under 100 MB, never posts results back and identifies itself honestly, but this is a grey area. See [ADR 0003](docs/adr/0003-speed-test-uses-cloudflare-public-endpoints.md).
- **Speed Test and Ookla:** Ookla's licence allows personal, non-commercial use only and uploads each result to Ookla. See [ADR 0004](docs/adr/0004-ookla-speedtest-is-optional-and-user-installed.md).
- **A Speed Test moves a lot of data:** roughly 0.25 GB on a 100 Mbps line. Techbench asks first on a metered connection.

## Project layout

| Path | Contents |
| --- | --- |
| `crates/toolbox-core/` | Every tool's behaviour, with no dependency on Tauri or the UI. The tests live here. |
| `src-tauri/` | The Tauri shell that exposes the core to the UI. |
| `src/` | The React and TypeScript UI. |
| `docs/` | Architecture decisions (`adr/`) and the design system. |
| `.scratch/` | The specs and tickets the tools were built from. |

Domain vocabulary is in [CONTEXT.md](CONTEXT.md).

## Vendor data

LAN Scan names a device's vendor from the IEEE OUI registry, compiled in from `crates/toolbox-core/data/oui.tsv`, so lookups work offline. To refresh it, run `python scripts/update-oui.py` (needs internet) and rebuild.

## Roadmap

Ideas, not promises, taken from what the specs deliberately left out:

- Show the Wi-Fi link speed and signal in Speed Test results
- Whois, VLSM subnet splitting, and offline utilities (hash, base64, password)
- IPv6 LAN Scan, UDP port checks, user-defined port presets
- A run history and a multi-target monitor with alerts
- Remote agents, so a tool can run from another machine
- Signed installers and auto-update

## License

MIT, see [LICENSE](LICENSE). Copyright (c) 2026 Venuel Rey R. Gallarte.
