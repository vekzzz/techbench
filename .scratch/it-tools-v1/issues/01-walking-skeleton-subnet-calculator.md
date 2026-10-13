# 01 — Walking skeleton: Subnet Calculator end to end

**What to build:** The Technician opens IT Tools and sees a sidebar listing the seven Tools (Ping, Traceroute, DNS Lookup, Port Check, LAN Scan, IP Info, Subnet Calculator) plus Saved Targets. Subnet Calculator works completely; every other entry shows a "coming soon" placeholder. This ticket establishes the application's three-layer shape from the spec (a toolbox core library with no Tauri or UI dependency, a thin Tauri 2 shell, and a React + TypeScript UI styled with Tailwind and shadcn/ui) and the core's test harness that every later ticket builds on.

Spec: user stories 1, 2, 6, 7, 74–79; Implementation Decisions › Shape of the application, Toolbox core, Per-Tool rules › Subnet Calculator.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent
**Category:** enhancement

- [x] The app launches on Windows in dev mode showing the sidebar with the seven Tools and Saved Targets; unbuilt entries show a placeholder.
- [x] The toolbox core is a separate library with no dependency on Tauri or the UI; the Tauri shell only forwards to it.
- [x] Subnet Calculator accepts `address/prefix` or an address plus dotted mask, and shows network, broadcast, first and last usable, usable host count, prefix, dotted mask and wildcard mask.
- [x] A contains-check reports whether a second address is inside the subnet.
- [x] /31 yields two usable addresses and no broadcast; /32 yields one usable address.
- [x] IPv6 prefixes show network, prefix, first and last address and address count, with no broadcast or wildcard.
- [ ] Invalid input shows a clear inline message and never crashes.
- [x] Results update as the Technician types.
- [ ] Switching to another sidebar entry and back keeps Subnet Calculator's inputs and results (Tool state lives above the Tool screens).
- [ ] Dark theme by default with a light toggle; addresses and results use a monospace font.
- [x] Table-driven core tests cover /0–/32, the /31 and /32 edge cases, dotted-mask input, the contains-check, IPv6 prefixes and invalid input, and run with the project's standard test command.

## Comments

**2026-10-13 — implementation pass (Claude).** Toolbox core crate plus Tauri shell plus React UI built test-first; 19 core tests pass via `npm test` (`cargo test --workspace`), clippy clean. Verified in the running dev app: sidebar, placeholders, live Subnet Calculator results and the contains-check. Still to confirm by hand: the inline invalid-input message, the state surviving a sidebar switch, and the light-theme toggle. The core spells out the IPv6 /0 count (2^128) as text, because u128 cannot hold it.
