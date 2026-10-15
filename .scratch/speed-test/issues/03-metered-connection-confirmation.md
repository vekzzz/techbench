# 03 — Speed Test: metered connection confirmation

**What to build:** Before a Run starts, Speed Test asks Windows whether the connection is metered, roaming or over its data limit, and if so asks the Technician to confirm, like LAN Scan's large-range confirmation. A Run uses about 0.25 GB on a 100 Mbps line and up to about 2.5 GB at 1 Gbps, which matters on a hotspot or a capped LTE backup line.

Spec: [spec.md](../spec.md) — user story 11; Implementation Decisions › Network adapter seam (`connection_cost`).

**Blocked by:** 01 — Speed Test: latency and download.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Seam gains `connection_cost()` returning metered, unmetered, roaming, over-limit or unknown; fake scripts it per test; real uses the `windows` crate (`NetworkInformation`/`ConnectionCost`), which becomes a direct dependency of the core (it is already in `Cargo.lock` through Tauri). No elevation needed.
- [x] Fixed or Variable cost type, roaming, or over the limit counts as "needs confirmation"; unmetered and unknown start straight away.
- [ ] When confirmation is needed, the screen explains why and roughly how much data a Run uses, and starts only when the Technician confirms; cancelling starts nothing.
- [x] A missing connection profile (no internet) is handled without a crash and falls through to the No internet ending.
- [x] Core tests with the fake cover each cost classification; an opt-in or Windows-only smoke test proves the real call returns a value on this PC.
- [ ] The confirmation is checked manually in the running app.

## Comments

**2026-10-15 — implementation pass (Claude).** `Network::connection_cost()` (Unmetered / Metered / Roaming / OverLimit / Unknown), real implementation on the `windows` crate (`Networking_Connectivity`, now a direct dependency; same 0.62 as Tauri's), `speed_test::check_connection` words the warning. Smoke test on this PC returned `Unmetered`. The screen asks before starting through the `speed_test_cost_warning` command; **not seen in the running app**, and a metered connection has not been tried (needs a hotspot or Windows' "Set as metered connection" toggle).
