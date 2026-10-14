# 07 — Continuous Ping

**What to build:** In Ping, the Technician starts a Continuous Ping against one to three Targets side by side (for example the default gateway and 8.8.8.8) and watches a live latency graph and running statistics for each until they stop it. Each Target is its own Run.

Spec: user stories 3, 4, 32–38; Implementation Decisions › Per-Tool rules › Continuous Ping, UI.

**Blocked by:** 05 — Ping; 06 — IP Info.

**Status:** ready-for-agent
**Category:** enhancement

- [x] Continuous Ping repeats until stopped, using the same interval and timeout as Ping.
- [x] One to three Targets run side by side, each as its own Run; a fourth cannot be added.
- [x] The default gateway (from the adapter carrying the default route) is offered as a quick Target.
- [x] Each Target has a live latency graph with timeouts marked distinctly from slow replies.
- [x] Each Target shows running loss percentage, minimum/average/maximum and jitter, where jitter is the mean absolute difference between consecutive successful round-trip times.
- [x] Continuous Ping keeps running when the Technician switches Tools.
- [x] The Result can be copied while running and after stopping.
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

**2026-10-14 — review follow-up (Claude).** Fixed:
- **"Constant memory" was not true:** a flapping link grew the lost-runs list by one entry per episode. The core now keeps the latest 200 runs and counts the older ones (`older_lost_runs`); Copy as text says "... and N earlier lost runs not listed".
- **Loss of 1 echo in 1,000 showed "0% loss".** It now shows "<1%" (`loss_label()`, sent to the UI as `lossLabel`); the Loss figure turns red whenever any echo was lost.
- **Remove left the slot's old Target and graph behind;** it now clears the slot (`clear()` on the run store). **Start all no longer restarts slots that are already running** (which silently reset their totals).
- **`Runs::text` no longer holds the Runs lock while formatting.**
- **Duplication:** Ping's summary now reuses `PingStatistics::describe`; the default echo request lives in `ping::default_echo_to`; `RunEnding` matches list every arm instead of wildcards. Graph label says "latest echoes".

Accepted or deferred:
- Continuous Ping uses the fixed Ping defaults (1 s, 2000 ms, 32 bytes), not the values set in Once mode.
- The three-slot cap lives in the store, the screen and the array of stores.
- "Use default gateway" can overwrite the Target text of a slot whose Run is going (the Run itself is unaffected).
- The graph marks every non-reply (timed out, unreachable, fragmentation needed) as lost.
- The totals labels and red highlight are worded in the UI; the summary line comes from the core.
- Copy in the instant between a Run opening and finishing can say "Nothing to copy yet".
- The mid-Run test in `port_check.rs` relies on one `yield_now()`; a paused-clock version would be sturdier.

**2026-10-14 — GUI check (Claude).** Checked in the running app on 2026-10-14: the dev build (UI served by vite) driven through WebView2's DevTools protocol, so clicks and typing went into the app's own page, never another window; results read from the page, screenshots from the page, Copy checked on the Windows clipboard.
- Three slots (127.0.0.1, 192.0.2.1, 8.8.8.8) ran together; with three shown, Add Target is disabled.
- Graphs: replies draw a line; 192.0.2.1 (a reserved test address that never answers) drew only red bars and showed Loss 100% in red.
- On Traceroute for 4 s the Runs kept going (Sent 32 → 36 on return).
- Copy while running gave "Running for 16s: 17 sent…"; after Stop all, Copy gave the stopped Result with its lost run `#1-#18  13:07:47  18`.
