# Tauri with native Windows network APIs

IT Tools is a Tauri 2 desktop app (Rust core, React + TypeScript UI) that performs ICMP, ARP, DNS and TCP work through native Windows APIs from Rust, rather than an Electron app shelling out to `ping.exe` / `tracert.exe` / `nslookup.exe`. Parsing those commands' output is fragile because it is localised by Windows display language, and Node cannot send ICMP without native addons. Native APIs also let every v1 Tool run without Administrator rights.

## Considered Options

- **Electron + Node, shelling out to Windows commands** — all-TypeScript, but locale-dependent parsing and ~150 MB installs.
- **CLI only** — fastest to build, but no live graphs for Continuous Ping or clickable Results.
- **Local web app (localhost server + browser)** — two processes to keep alive for no gain on a single PC.
