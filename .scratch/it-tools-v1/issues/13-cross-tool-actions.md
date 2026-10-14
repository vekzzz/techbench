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

## Comments

**2026-10-14 — implementation pass (Claude).** UI only, so there are no core tests, matching the spec's "UI is verified manually". `src/lib/follow-lead.ts` holds the behaviour and `src/components/action-menu.tsx` the menu (a Radix dropdown from shadcn).

- **Where the menu is:** each LAN Scan Device and Traceroute Hop (Ping, Continuous Ping, Traceroute, Port Check, DNS Lookup, Save as Target) and each Saved Target (the same without Save as Target). A Hop with no address shows no menu.
- **What an action does:** puts the address into the Tool's own input store with the IP version back on Auto, switches to the Tool and starts it through the same `start` the Start button uses. Ping first loads its defaults if its screen was never opened. Port Check keeps whatever ports are typed; with none, it shows its usual "enter at least one port" message. DNS Lookup on an address is a PTR lookup, which the core already does.
- **Continuous Ping:** uses an empty slot that is not running, else a slot whose Run has ended, and shows more slots when needed. With all three running, it switches to Continuous Ping and shows a dismissible notice: "Three Continuous Pings are already running. Stop one first, then try again."
- **Save as Target** opens the Saved Targets screen with the form filled in (address, and the hostname as label when known) for the Technician to confirm.

**Not verified in the running app:** none of these actions has been clicked through. Please try one of each, and the three-running notice.

**2026-10-14 — review follow-up (Claude).** Not changed, for you to decide:
- **Continuous Ping reuses a slot whose Run has ended**, or one with a typed but unstarted Target, if no slot is empty; only running slots are protected. That follows "when fewer than three are running".
- **Port Check from an action** shows its "enter at least one port" message when no ports are typed, as the ticket says ("as if the Technician had typed the address and started it").
- The names `followLead` and `LeadAction` are not in CONTEXT.md; they are code-level only, and the screens say "Actions".
