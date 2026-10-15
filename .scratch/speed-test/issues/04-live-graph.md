# 04 — Speed Test: live graph

**What to build:** While a Run is going, the screen draws a live graph of speed samples with a Loaded Latency line, in the style of Continuous Ping's graph, so the Technician can watch the line saturate and latency spike.

Spec: [spec.md](../spec.md) — user story 3; Implementation Decisions › UI.

**Blocked by:** 02 — Speed Test: upload and bufferbloat.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] The core emits a speed sample about once a second per direction and each Loaded Latency probe as events; they are also in the Run's Progress so a screen opened mid-Run can draw them.
- [ ] The graph is an SVG in the style of Continuous Ping's: speed in Mbps as a filled line, Loaded Latency as a second line on its own axis, the idle latency as a reference line, and the warm-up window visibly distinguished from the measured window.
- [ ] Download and upload phases are labelled on the time axis.
- [ ] The graph stays after the Run ends, including a Stopped or Interrupted one, showing what was measured.
- [ ] Core tests cover the emitted samples; the graph itself is verified manually in the running app (dev build driven through WebView2's DevTools protocol, as in v1).

## Comments

**2026-10-15 — implementation pass (Claude).** The core emits a speed sample each second and every loaded probe as events, and the Result is readable mid-Run. `SpeedGraph` draws one timeline, download then upload: speed, latency under load, idle latency as a dashed line, warm-up bands. It builds and type-checks; **not looked at in the running app**. Deviation: the graph is one timeline for both directions rather than a graph per phase.

Correction: samples and loaded probes are events only; `Progress` holds the Result, not the samples, so a screen opened mid-Run cannot redraw what it missed (the store keeps them while the app runs). Event tests exist (first sample, loaded probes), but the graph itself is unverified.
