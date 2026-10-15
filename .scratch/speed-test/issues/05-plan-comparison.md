# 05 — Speed Test: plan comparison

**What to build:** The Technician can enter the speeds their ISP plan promises, and the Result says how close the measured speeds came, for example "38 Mbps (19% of 200 Mbps plan)". This is what turns a number into ISP evidence.

Spec: [spec.md](../spec.md) — user story 7; Implementation Decisions › Shape, Measurement.

**Blocked by:** 02 — Speed Test: upload and bufferbloat.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] Optional "Plan: ___ / ___ Mbps" inputs (download, upload) on the screen, kept in the app-level store until the app closes and never written to disk.
- [ ] Plan speeds are part of the Run's request; with a plan set, the Result adds "% of plan" for that direction, on screen and in Copy as text.
- [x] Either or both directions may be left blank; blank means no comparison for that direction.
- [x] Invalid input (zero, negative, non-numeric) is rejected with a clear message before the Run starts.
- [x] Core tests cover percentage maths (including over 100%), one-sided plans and golden Copy as text with and without a plan.

## Comments

**2026-10-15 — implementation pass (Claude).** `SpeedTestRequest::validate` reads the plan speeds as Mbps (blank for none; zero, negative, non-numeric, infinite and NaN rejected with a message naming the direction); `plan_percent` rounds to a whole percent and can pass 100; Copy as text reads `38.0 Mbps (19% of 200 Mbps plan)`. The inputs live in the app store (never on disk). The inputs are built but **not looked at in the running app**.
