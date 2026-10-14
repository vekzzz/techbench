# 07 — Continuous Ping

**What to build:** In Ping, the Technician starts a Continuous Ping against one to three Targets side by side (for example the default gateway and 8.8.8.8) and watches a live latency graph and running statistics for each until they stop it. Each Target is its own Run.

Spec: user stories 3, 4, 32–38; Implementation Decisions › Per-Tool rules › Continuous Ping, UI.

**Blocked by:** 05 — Ping; 06 — IP Info.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Continuous Ping repeats until stopped, using the same interval and timeout as Ping.
- [ ] One to three Targets run side by side, each as its own Run; a fourth cannot be added.
- [x] The default gateway (from the adapter carrying the default route) is offered as a quick Target.
- [ ] Each Target has a live latency graph with timeouts marked distinctly from slow replies.
- [x] Each Target shows running loss percentage, minimum/average/maximum and jitter, where jitter is the mean absolute difference between consecutive successful round-trip times.
- [ ] Continuous Ping keeps running when the Technician switches Tools.
- [ ] The Result can be copied while running and after stopping.
- [x] Core tests with the fake adapter and controllable clock cover loss, minimum/average/maximum and jitter over scripted replies and timeouts, three concurrent Runs, and golden Copy as text.

## Comments

**2026-10-13 — note from ticket 04 (Claude).** "The Result can be copied while running" needs a shared way to get the Copy as text of a Run in progress (story 83). Port Check needs it as well: a 1,024-port Run lasts about a minute, and right now its text only exists once the Run finishes. Build the mechanism here in a form every Tool can use, and apply it to Port Check in the same change.

**2026-10-13 — note from ticket 06 (Claude).** Call `ip_info::find_default_route(network)` to get the Default Route Adapter. The routing table decides it, so a full-tunnel VPN is found too. Offer its first IPv4 gateway as the quick Target. Watch out for an adapter with only an IPv6 link-local gateway (`fe80::…`): `Adapter.gateways` carries no zone id, so pinging it can fail. Prefer an IPv4 gateway, and leave the quick Target out rather than offer a link-local address without its zone.

**2026-10-14 — implementation pass (Claude).** Built test-first at seam S1 over the fake network (S2) and tokio's paused clock: 7 Continuous Ping core tests, 3 default-gateway tests, 2 Progress tests, 1 real-network smoke test.

Shared mechanism (from ticket 04's note): `run::Progress<R>` holds a Run's Result as it builds up, readable at any moment, with `RunEnding::Running`. Port Check, Ping and Continuous Ping all keep their Result in one (`run_with_progress`), so their Copy as text, with a "Still running" summary, works mid-Run. The shell's `run_text(id)` command reads it; `CopyResult` takes a `runId`.

Continuous Ping:
- **Constant memory:** running totals (`PingStatistics`: sent, received, min/avg/max, jitter as the mean absolute difference between consecutive replies) plus one entry per run of lost echoes.
- **Copy as text** groups consecutive losses with when each began: `#2-#3  14:05:10  2`.
- **Pacing loop shared with Ping** (`ping::send_paced`).
- **`ip_info::default_gateway()`** gives the quick Target: the IPv4 gateway, else a global IPv6 one; link-local IPv6 is left out (no zone), as noted from ticket 06.
- **Ping screen:** Once / Continuous toggle; Continuous has up to three Target slots, each its own Run and store, with Start all / Stop all, Add Target, Use default gateway, totals and an SVG graph (last 60 echoes, lost echoes as red bars).

Verified: all tests; `Use default gateway` filled 192.168.1.1 in the running app. **Not verified visually**: the live graph, three slots running together, switching Tools mid-Run, and Copy while running in the UI. The GUI check was abandoned: my automation sent keystrokes to the wrong window and corrupted `toolbox.ts` (fixed; tsc clean). Those four boxes stay unticked for a manual look.
