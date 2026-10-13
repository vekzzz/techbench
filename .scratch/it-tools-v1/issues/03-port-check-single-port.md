# 03 — Port Check, single port (first networked Tool)

**What to build:** The Technician enters a Target and one TCP port in Port Check and sees whether it is Open, Closed or Filtered, with the connect time for Open, and the Resolved Address the Run used. Copy as text produces the standard header, the row and a summary line.

As the first Tool that touches the network, this ticket also introduces the shared machinery every later Tool reuses:

- the Run model: starting a Run returns a Run id, events stream to the UI, stopping by Run id ends with a stopped Result holding everything collected so far, and Runs keep going when the Technician switches Tools;
- the network adapter trait (test seam S2) with a scripted fake and a real implementation covering TCP connect and system name resolution;
- Target validation and the Resolved Address policy, with the IPv4/IPv6 force toggle;
- the Copy as text formatter;
- the shared Target input used by every Tool that takes a Target.

Spec: user stories 3, 10–16, 54, 55, 80–83; Implementation Decisions › Toolbox core, Network adapter, Per-Tool rules › Port Check, Tauri shell.

**Blocked by:** 01 — Walking skeleton: Subnet Calculator end to end.

**Status:** ready-for-agent
**Category:** enhancement

- [x] The Target input accepts a hostname, IPv4 address or IPv6 address; an invalid Target is rejected with a clear message before the Run starts.
- [x] A hostname with several addresses uses its first IPv4 address unless IPv6 is forced; the Result always states the Resolved Address.
- [x] A hostname that fails to resolve produces a resolution-failure Result, not Filtered.
- [x] An accepted connection is Open (with connect time), a refused one is Closed, and a timeout (2 s) or unreachable error is Filtered.
- [x] A Run can be stopped; a Run keeps going when the Technician switches Tools, and its Result is there on return.
- [x] Copy as text starts with a header naming the Tool, Target, Resolved Address, local timestamp with UTC offset, and the source PC's hostname and IP, followed by aligned plain-text rows and a summary line, with no Markdown.
- [x] Core tests drive the toolbox core with the fake adapter: Target validation, the Resolved Address policy, Open/Closed/Filtered classification, resolution failure, stopping a Run, and golden Copy as text.
- [x] Smoke tests with the real adapter: a TCP listener the test opens on localhost reports Open; an unused localhost port reports Closed.

## Comments

**2026-10-13 — implementation pass (Claude).** Built test-first at seam S1, with a scripted fake network at S2: 20 core tests plus 3 smoke tests against the real network.
- **New core modules:** `target` (Target parsing and the Resolved Address policy), `run` (`RunHeader`, `RunEnding`, the Copy as text helpers, and a re-exported `CancellationToken`), `network` (the `Network` trait and the real `SystemNetwork`), and `port_check`.
- **The shell:** a registry of Runs in progress (`stop_run(id)`), and `start_port_check`, which validates first and then streams updates over a Tauri channel.
- **The UI:** a store slice that owns the Run (so it survives switching Tools), the shared `TargetInput` with its Auto/IPv4/IPv6 toggle, and the Port Check screen.

Found while testing on this PC:
- **Closed ports looked Filtered.** Windows retries a SYN even after a refusal, so a closed port took longer than the 2 s timeout to report. The real adapter now limits SYN retransmissions to one (`SIO_TCP_INITIAL_RTO`); see the review follow-up below for the measurements.
- **Resolver errors ended in "(os error 11001)".** That suffix is now stripped.
- **JetBrains Mono ligatures drew `..` and `::` misleadingly.** Ligatures are now off for monospace text.

Verified in the running app:
- localhost:1420 with Auto showed 127.0.0.1 Closed. Vite listens on `::1` only, so that is correct. Forcing IPv6 showed `::1` Open in 1 ms, and the clipboard text matched the golden format.
- Stop showed "Stopped before any port answered."
- A Run against a silent address finished as Filtered while the Technician was on another Tool.
- A malformed Target showed an inline error, and an unknown host showed a resolution failure.

**2026-10-13 — review follow-up (Claude).** The Spec review caught a real problem: turning SYN retransmissions off entirely made Windows abandon a silent Target at its 1 s initial RTO. That cut the 2 s timeout in half and never resent a lost SYN. Measured on this PC:

| Setting | Refused (localhost) | Silent (10.255.255.1) |
| --- | --- | --- |
| Windows default | 2042 ms | still waiting at 6 s |
| no retransmissions | 0 ms | gives up at 1012 ms |
| one retransmission (chosen) | 513 ms | resends at about 1 s; the 2 s timeout ends it as Filtered |

An OS-level timeout now also maps to TimedOut instead of Unreachable. Smaller cleanups from the Standards review:
- Stop handling uses `CancellationToken::run_until_cancelled`.
- The store has one `stopIfRunning` helper.
- The shell's `Family` is renamed `AddressFamilyInput`.
- The unused `invalid` prop on `TargetInput` is gone.
- `Target::parse` is now private.
- The tests share a `run_then_stop` helper.

Deferred on purpose:
- Copying a Run that is still running. A single-port check takes 2 s at most; tickets 07, 08 and 10 need mid-run copying and will add it.
- A Target-plus-IP-version type, and generic Run handling in the UI store. Both wait for Ping (ticket 05), the second Tool to need them.
