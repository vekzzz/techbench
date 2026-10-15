# 01 — Speed Test: latency and download

**What to build:** A new Speed Test Tool in the sidebar. The Technician presses Start and watches the Run measure idle latency and jitter, then download speed, with a live Mbps figure and the current phase. The Result copies as plain text with a header naming the Speed Test Server, time, PC and Adapter, and public IP/ISP. This slice also lays the whole path end to end: seam operations, real Cloudflare implementation, fake, shell command, screen, store.

Spec: [spec.md](../spec.md) — user stories 1, 2, 4, 6, 8, 9, 10, 12–18; Implementation Decisions › Network adapter seam, Measurement, Endings, Copy as text, UI. Decision record: [ADR 0003](../../../docs/adr/0003-speed-test-uses-cloudflare-public-endpoints.md).

**Blocked by:** none (builds on shipped v1 tickets 01–13).

**Status:** ready-for-agent
**Category:** enhancement

- [x] Seam gains `speed_probe` and `speed_download` with a scripted fake (transfer rate and latency per call, paused clock) and a real Cloudflare implementation on `ureq` inside `spawn_blocking`; every request stays under 100 MB, sends an honest User-Agent, no Origin/Referer, and posts nothing back.
- [x] Idle latency is the median of 20 probes (round trip − server time); jitter is the mean absolute difference between consecutive samples.
- [ ] Download uses 4 connections for 10 s, ignores the first 2 s, and reports Mbps over the remaining window; the live figure updates while it runs.
- [x] The Run reports data used, the Speed Test Server (location code and address) and the address family used.
- [x] Same IPv4/IPv6 toggle and rule as other Tools.
- [x] Public IP/ISP comes from the existing lookup in parallel with the idle latency phase; its failure shows in the header and does not end the Run.
- [x] Header names the Adapter the Run went out through (Default Route Adapter).
- [x] Endings: Completed, Stopped (keeps what was measured), No internet (fails within about 5 s), Rate limited (shows `Retry-After`, no automatic retry), Interrupted (finished phases kept, the rest marked not measured).
- [ ] Only one Speed Test Run at a time; Start is disabled while one runs; other Tools run freely; switching Tools never stops it.
- [ ] Copy as text works while running and after stopping, in the spec's header shape.
- [ ] The Server address has the action menu with Ping, Continuous Ping and Traceroute.
- [x] The doc comment on `Network::public_ip` and the v1 "only external call" wording are updated to "Tools whose subject is the internet may call out".
- [x] Core tests with the fake adapter and paused clock cover idle median and jitter, the warm-up window, 4-connection aggregation, data used, family forcing, each ending, public-lookup failure, and golden Copy as text. An opt-in `speed_test_on_this_pc.rs` smoke test runs real Cloudflare with a 1 MB download and two probes.

## Comments

**2026-10-15 — implementation pass (Claude).** Built at seam S1 over the fake network (S2) and tokio's paused clock: 17 core tests in `tests/speed_test.rs`, 2 opt-in smoke tests in `tests/speed_test_on_this_pc.rs` (real Cloudflare: location code and server time parsed, a 1 MB download arriving whole). Tests for the endings and Copy as text were written after the code they cover; the latency, jitter, download, data-used, family and header tests were red first.

Decisions made while building, not in the spec:
- A phase that does not run to its end (Stopped, Interrupted) leaves its speed **unmeasured** rather than reporting a partial figure; its bytes still count towards data used.
- The Speed Test Server address is the first address `speed.cloudflare.com` resolves to for the chosen family, because `ureq` does not report the address it connected to. It can differ from the one used if DNS rotates.
- Server time is the first `dur=` in `Server-Timing` (as Cloudflare's own library reads it); latency is measured to the first byte of the response, so the first probe includes the TLS handshake and the median absorbs it.
- The public lookup runs in parallel with the 20 probes, so a slow lookup (5 s timeout) holds the download back by up to that long.
- Live speed samples are one per second including the warm-up; the 10 s tick is dropped in favour of ending the phase.

Verified: `cargo test --workspace` (264 passed, 5 ignored), `cargo clippy`, `tsc`, `npm run build`, and the two smoke tests against real Cloudflare (HKG, ~50 ms, 3 ms server time). **Not verified in the running app**: the live figure, Stop, Copy while running, the action menu on the Server address, the IPv4/IPv6 toggle on a real Run, and No internet within about 5 s. Those boxes stay unticked for a manual look. `dns_client_on_this_pc::a_truncated_answer_is_asked_again_over_tcp` failed once under the parallel full run and passed alone three times; it is unrelated to Speed Test.
