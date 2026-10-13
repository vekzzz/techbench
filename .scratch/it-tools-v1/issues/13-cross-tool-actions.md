# 13 — Cross-Tool actions

**What to build:** Any LAN Scan Device, Traceroute Hop or Saved Target offers an action menu (Ping, Continuous Ping, Traceroute, Port Check, DNS Lookup, Save as Target). Choosing an action switches to that Tool with the address filled in and the Run started, so following a lead is one step.

Spec: user stories 24, 25; Implementation Decisions › UI.

**Blocked by:** 04 — Port Check: lists, ranges and Port Presets; 07 — Continuous Ping; 08 — Traceroute; 09 — DNS Lookup; 11 — LAN Scan: hostnames, vendors, sort and filter; 12 — Saved Targets.

**Status:** ready-for-agent
**Category:** enhancement

- [ ] Devices, Hops and Saved Targets each offer Ping, Continuous Ping, Traceroute, Port Check and DNS Lookup; Devices and Hops also offer Save as Target.
- [ ] Choosing an action switches to that Tool with the address filled in and the Run started, behaving exactly as if the Technician had typed the address and started it.
- [ ] A Hop with no address (all probes silent) offers no actions.
- [ ] Continuous Ping adds the address alongside running Targets when fewer than three are running; otherwise the Technician is told to stop one first.
- [ ] DNS Lookup on an address performs a PTR lookup.
- [ ] Save as Target from a Device pre-fills the label with its hostname when known.
