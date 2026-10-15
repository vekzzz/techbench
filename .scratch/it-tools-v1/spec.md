# Spec: IT Tools v1 — the seven-Tool Windows toolbox

Status: ready-for-agent
Category: enhancement

Vocabulary follows [CONTEXT.md](../../CONTEXT.md). Respects [ADR 0001](../../docs/adr/0001-tools-run-only-on-technician-pc.md) (Tools run only on the Technician's PC) and [ADR 0002](../../docs/adr/0002-tauri-with-native-windows-network-apis.md) (Tauri with native Windows network APIs).

## Problem Statement

A Technician diagnosing network problems on Windows today juggles `ping`, `tracert`, `nslookup`, `ipconfig`, `Test-NetConnection`, `arp -a` and assorted websites. Each has its own syntax and output; `tracert` is slow because it waits four seconds per silent probe; the output changes with the Windows display language; and none of it remembers the addresses the Technician checks every day. Intermittent drops are hard to chase because `ping -t` gives no running loss or jitter figures and cannot compare the gateway against the internet side by side. Finding devices on a subnet with ping alone misses most Windows PCs, whose firewalls drop echo requests by default. When the Technician pastes evidence into a helpdesk ticket, the paste rarely says when the test ran, from which PC, or which IP a hostname actually resolved to, so the next person cannot trust or reproduce it.

## Solution

IT Tools is a small Windows desktop app that puts seven Tools in one window: Ping (with Continuous Ping), Traceroute, DNS Lookup, Port Check, LAN Scan, IP Info and Subnet Calculator. Everything runs on the Technician's own PC without Administrator rights and works offline except the public-IP lookup. A sidebar switches between Tools; each Tool keeps its state and keeps running in the background. The Technician can store Saved Targets with labels and notes and pick them in any Tool, and can jump from any Device, Hop or Saved Target straight into another Tool. Every Result can be copied as plain text with a header that records the Tool, Target, Resolved Address, time and source PC, ready to paste into a ticket.

## User Stories

### Shell and general behaviour

1. As a Technician, I want all seven Tools listed in a sidebar, so that I can move between diagnostics without opening separate programs.
2. As a Technician, I want each Tool to keep its inputs and Result when I switch to another Tool, so that I do not lose work by looking elsewhere.
3. As a Technician, I want a Run to keep going when I switch away from its Tool, so that a Continuous Ping keeps collecting while I run a Traceroute.
4. As a Technician, I want several Runs of different Tools active at the same time, so that I can correlate symptoms while they happen.
5. As a Technician, I want the app to open without a UAC prompt, so that launching it is as quick as opening a command prompt.
6. As a Technician, I want a dark theme by default with a light-theme toggle, so that it is comfortable in a dim server room and readable on a projector.
7. As a Technician, I want addresses, ports and Results shown in a monospace font, so that columns line up and digits are easy to compare.
8. As a Technician, I want every Tool except IP Info's public lookup (and the later Speed Test) to work with no internet connection, so that I can diagnose isolated or broken networks.
9. As a Technician, I want to install the app from a single Windows installer, so that I can set it up on a new PC in a minute.
10. As a Technician, I want every Run to be stoppable, so that I am never stuck waiting on a long scan or trace.

### Targets and Resolved Addresses

11. As a Technician, I want to enter a hostname, an IPv4 address or an IPv6 address as a Target in any Tool that takes one, so that I can test whatever I have in hand.
12. As a Technician, I want an invalid Target rejected with a clear message before the Run starts, so that typos do not look like network failures.
13. As a Technician, I want a hostname that resolves to several addresses to use an IPv4 address by default, so that behaviour matches what most of my network uses.
14. As a Technician, I want to force IPv4 or IPv6 for a Run, so that I can test each address family explicitly.
15. As a Technician, I want every Result to state the Resolved Address it actually used, so that "which IP did it hit?" is never in doubt.
16. As a Technician, I want a hostname that fails to resolve reported as a resolution failure, not as a timeout, so that I look at DNS rather than routing.

### Saved Targets

17. As a Technician, I want to save a Target with a label and an optional note, so that I can refer to "Core switch — VLAN 20, rack B" instead of remembering 10.0.0.1.
18. As a Technician, I want to pick a Saved Target in any Tool that takes a Target, so that I do not retype frequent addresses.
19. As a Technician, I want to edit and delete Saved Targets, so that the list stays accurate as the network changes.
20. As a Technician, I want Saved Targets to persist between app launches, so that the list is there every morning.
21. As a Technician, I want to export my Saved Targets to a JSON file, so that I can back them up or move them to another PC.
22. As a Technician, I want to import Saved Targets from a JSON file, so that I can restore a backup or adopt a colleague's list.
23. As a Technician, I want to save a Target directly from a Device, a Hop or a typed Target, so that useful addresses I discover are one click from being kept.

### Cross-Tool actions

24. As a Technician, I want an action menu on any Device, Hop or Saved Target offering Ping, Continuous Ping, Traceroute, Port Check and DNS Lookup, so that I can follow a lead without retyping the address.
25. As a Technician, I want the chosen action to open the target Tool with the address pre-filled and the Run started, so that following a lead is a single step.

### Ping

26. As a Technician, I want Ping to default to 4 echoes of 32 bytes, 1 second apart, with a 2 second timeout, so that a quick check needs no setup.
27. As a Technician, I want to change the echo count, payload size and timeout, so that I can tailor the test.
28. As a Technician, I want a Don't Fragment toggle, so that I can probe path MTU, for example to prove a VPN MTU problem.
29. As a Technician, I want each echo shown as a row with sequence number, round-trip time, TTL and status, so that I can see exactly what happened to each packet.
30. As a Technician, I want "fragmentation needed", "destination unreachable" and "timed out" reported as different statuses, so that I can tell an MTU problem from a routing problem from a silent drop.
31. As a Technician, I want a summary of sent, received, loss percentage and minimum/average/maximum round-trip time, so that I can judge the link at a glance.

### Continuous Ping

32. As a Technician, I want a Continuous Ping that repeats until I stop it, so that I can catch intermittent drops.
33. As a Technician, I want to run Continuous Ping against up to three Targets side by side, so that I can compare the gateway with an internet address and tell a LAN problem from an ISP problem.
34. As a Technician, I want my default gateway offered as a quick Target in Continuous Ping, so that I can start the LAN-versus-ISP comparison instantly.
35. As a Technician, I want a live latency graph per Target, so that I can see spikes and drops as they happen.
36. As a Technician, I want timeouts visibly marked on the graph, so that drops stand out from slow replies.
37. As a Technician, I want running loss percentage, minimum/average/maximum and jitter per Target, so that I can quantify link quality for the ticket.
38. As a Technician, I want to stop a Continuous Ping and still copy its Result, so that I can attach the evidence I collected.

### Traceroute

39. As a Technician, I want Traceroute to probe up to 30 Hops with 3 probes per Hop and a 1 second timeout, so that it finishes much faster than `tracert`.
40. As a Technician, I want Traceroute to stop as soon as the Target answers, so that I do not wait on hops that do not exist.
41. As a Technician, I want each Hop shown as it is discovered with its number, address and three round-trip times, so that I see progress immediately.
42. As a Technician, I want silent probes shown as `*`, so that I can tell a filtering router from a slow one.
43. As a Technician, I want Hop hostnames filled in after the Hop appears, so that reverse DNS never slows the trace.
44. As a Technician, I want Traceroute to work over IPv6, so that I can diagnose dual-stack paths.

### DNS Lookup

45. As a Technician, I want to query A, AAAA, CNAME, MX, TXT, NS, PTR and SOA records, so that I can investigate any common DNS issue.
46. As a Technician, I want DNS Lookup to use my PC's configured DNS server by default, so that I see what my machine is being told.
47. As a Technician, I want to choose another DNS server, including quick picks such as 8.8.8.8 and 1.1.1.1 or a typed internal server, so that I can compare internal and public answers.
48. As a Technician, I want entering an IP address to perform a reverse (PTR) lookup automatically, so that I do not have to build `in-addr.arpa` names.
49. As a Technician, I want each answer record shown with its type, value and TTL, so that I can spot stale or misconfigured records.
50. As a Technician, I want the Result to say which server answered and how long it took, so that I can compare resolvers fairly.
51. As a Technician, I want "name does not exist", "server failure", "no records of this type" and "no response" reported distinctly, so that I know whether the problem is the record, the zone or the server.

### Port Check

52. As a Technician, I want to check several TCP ports on one Target in one Run, entered as a list and/or ranges such as `22, 80, 8000-8010`, so that I can test a service set in one go.
53. As a Technician, I want built-in Port Presets for Windows, Directory, Web, Remote, Mail, Database and Printing, so that common checks need no typing.
54. As a Technician, I want each port reported as Open, Closed or Filtered, so that I know whether the service is listening, the host is refusing, or a firewall is dropping traffic.
55. As a Technician, I want the connect time shown for Open ports, so that I can spot a slow service.
56. As a Technician, I want a service name hint beside each well-known port, so that I do not have to remember that 5985 is WinRM.
57. As a Technician, I want ports checked concurrently with a 2 second timeout, so that a Preset completes in about two seconds even when ports are Filtered.

### LAN Scan

58. As a Technician, I want LAN Scan to pre-fill the range from my active network adapter, so that scanning my own subnet takes one click.
59. As a Technician, I want to edit the range as CIDR or as a start–end range, so that I can scan any part of the network.
60. As a Technician, I want LAN Scan to use ARP as well as ping on my own subnet, so that Windows PCs that drop ping are still found.
61. As a Technician, I want Devices that answer ARP but not ping flagged as "no ping reply", so that I understand why they would not answer a Ping.
62. As a Technician, I want each Device shown with IP, hostname, MAC, vendor and response time, so that I can identify what it is.
63. As a Technician, I want the vendor derived from the MAC offline, so that I can tell an HP printer from a Ubiquiti access point without internet access.
64. As a Technician, I want MAC and vendor left blank, with an explanation, for Devices on other subnets, so that I know it is a limitation of ARP and not a bug.
65. As a Technician, I want ranges up to a /22 to scan freely, larger ranges up to a /16 to require my confirmation, and anything larger refused, so that I cannot flood the network by accident.
66. As a Technician, I want Devices to appear as they are found with a progress indicator, so that I am not staring at a blank screen.
67. As a Technician, I want a /24 to finish in a few seconds, so that LAN Scan is something I run casually.
68. As a Technician, I want to sort Devices by IP (numerically), hostname or vendor and filter them by text, so that I can find one Device in a busy subnet.

### IP Info

69. As a Technician, I want to see each network adapter's name, up/down state, IPv4 and IPv6 addresses with prefixes, gateway, DNS servers, MAC, and DHCP state with lease times, so that I have the full `ipconfig /all` picture in a readable form.
70. As a Technician, I want active adapters listed first and inactive ones collapsed, so that the relevant adapter is easy to find among virtual ones.
71. As a Technician, I want my public IP with ISP name and city, so that I know which internet line I am on at a dual-WAN site.
72. As a Technician, I want a failed public lookup shown as an error while local details still display, so that IP Info is useful offline.
73. As a Technician, I want to refresh IP Info, so that I can watch a DHCP renewal or VPN connect take effect.

### Subnet Calculator

74. As a Technician, I want to enter `192.168.1.10/24` or an address with a dotted mask, so that I can use whichever form I was given.
75. As a Technician, I want network address, broadcast, first and last usable, usable host count, prefix, dotted mask and wildcard mask, so that I have every figure I need for configs and ACLs.
76. As a Technician, I want to check whether another address is inside the subnet, so that I can settle "is this IP in that range?" instantly.
77. As a Technician, I want /31 and /32 handled correctly (two usable point-to-point addresses; a single host), so that the calculator is right for router links and host routes.
78. As a Technician, I want IPv6 prefixes supported, so that I can work out IPv6 ranges too.
79. As a Technician, I want results to update as I type, so that the calculator feels instant.

### Copying Results

80. As a Technician, I want a Copy as text action on every Result, so that I can paste evidence into any ticket system.
81. As a Technician, I want the copied text to start with a header naming the Tool, Target, Resolved Address, timestamp and my PC's name and IP, so that readers know when, from where and against what the test ran.
82. As a Technician, I want the copied rows aligned as plain text and followed by a summary line, so that the paste is readable where Markdown does not render.
83. As a Technician, I want to copy the Result of a stopped or still-running Run, so that I can share partial evidence without waiting.

## Implementation Decisions

### Shape of the application

- Tauri 2 desktop app with three layers: a **toolbox core** Rust library that has no dependency on Tauri or the UI; a thin **Tauri shell** that exposes the core as commands and streams events to the UI; and a **React + TypeScript UI** styled with Tailwind and shadcn/ui.
- The app ships without an Administrator manifest. Every v1 Tool works unelevated.
- Packaging is Tauri's NSIS bundler, unsigned. SmartScreen's one-time "run anyway" prompt is accepted for now.
- UI language is English only.

### Toolbox core (test seam S1)

- One entry point per Tool. Each takes a typed request and a cancellation handle and returns a stream of events: progress events (an echo reply, a Hop, a Hop hostname arriving later, a Device found, a port outcome) followed by a final Result. Stopping a Run ends the stream with a Result marked as stopped that contains everything collected so far.
- Subnet Calculator is a plain synchronous function in the core, since it sends no traffic.
- **Target handling** lives in the core: validation of hostname/IPv4/IPv6 syntax, and the Resolved Address policy. A literal IP is used as-is. A hostname is resolved through the system resolver; the first IPv4 address is chosen unless the request forces IPv6, in which case the first IPv6 address is chosen. A resolution failure produces a distinct Result rather than a network error.
- **Result to text**: the core owns the Copy as text format so it is identical everywhere and testable. Header lines: Tool, Target, Resolved Address, local timestamp with UTC offset, source PC hostname and its IP on the adapter used. Then aligned plain-text rows, then one summary line. No Markdown.
- **Saved Target store** lives in the core and receives its storage folder from the caller. A Saved Target has an id, label, address and optional note. The file and the export format share one JSON shape carrying a format version. Import adds entries and skips any entry whose label and address both match an existing Saved Target.
- Run identity: each Run has an id so the shell can stop it. Continuous Ping against three Targets is three Runs shown together.

### Network adapter (test seam S2)

- A single trait through which the core touches the operating system. It covers:
  - ICMP echo to an address with TTL, payload size, Don't Fragment and timeout, returning reply (round-trip time, TTL, responding address), time exceeded (responding address, used by Traceroute), destination unreachable, fragmentation needed, or timeout.
  - TCP connect to an address and port with a timeout, returning accepted (with connect time), refused, or timeout/unreachable.
  - System name resolution of a hostname to its address list.
  - DNS query of a name and record type against a specific server, returning records with TTLs or a response code, with UDP first and TCP fallback on truncation.
  - Reverse lookup of an address to a hostname.
  - ARP resolution of an IPv4 address to a MAC, or none.
  - Adapter enumeration with the fields IP Info shows, including which adapter carries the default route.
  - Public IP lookup returning IP, ISP and city.
  - Clock and sleep, so Continuous Ping timing is controllable in tests.
- The real implementation uses the Windows ICMP helper API (IPv4 and IPv6 variants, which need no elevation), the Windows ARP request API, the Windows adapter-addresses API, ordinary TCP sockets, a Rust DNS client library for direct server queries, and one HTTPS call to an ipinfo-type service for the public lookup.
- A fake implementation returns scripted, deterministic answers and a controllable clock.

### Per-Tool rules

- **Ping**: defaults 4 echoes, 32-byte payload, 1 s interval, 2 s timeout; count, size and timeout editable; Don't Fragment toggle. Summary: sent, received, loss %, min/avg/max round-trip time.
- **Continuous Ping**: same interval and timeout as Ping; 1–3 Targets; per Target running loss %, min/avg/max and jitter. Jitter is the mean absolute difference between consecutive successful round-trip times. The default gateway from adapter enumeration is offered as a quick Target.
- **Traceroute**: ICMP echo with TTL increasing from 1; 3 probes per Hop; 1 s timeout per probe; at most 30 Hops; stops when the Resolved Address replies. Reverse lookups for Hop addresses run concurrently and arrive as later events. Works for IPv4 and IPv6.
- **DNS Lookup**: record types A, AAAA, CNAME, MX, TXT, NS, PTR, SOA. The default server is the first DNS server configured on the adapter carrying the default route; quick picks 8.8.8.8 and 1.1.1.1; any server may be typed. Like `nslookup`, DNS Lookup queries the server directly, so the hosts file is not consulted (Ping, Traceroute and Port Check do use the system resolver, which consults it). An IP entered as the query switches the record type to PTR. Outcomes distinguish NXDOMAIN, SERVFAIL, an empty answer, and no response.
- **Port Check**: TCP only. Port lists accept comma-separated ports and inclusive ranges, deduplicated and sorted, capped at 1,024 ports per Run. Up to 32 connects in parallel, 2 s timeout each. Accepted → Open; refused → Closed; timeout or unreachable → Filtered. Built-in Port Presets, not user-editable in v1:
  - Windows: 135, 445, 3389, 5985, 5986
  - Directory: 53, 88, 389, 636, 3268
  - Web: 80, 443, 8080, 8443
  - Remote: 22, 3389, 5900
  - Mail: 25, 465, 587, 993, 995
  - Database: 1433, 3306, 5432
  - Printing: 9100, 631, 515
  - A static table maps well-known ports to service name hints.
- **LAN Scan**: IPv4 only. The default range is the IPv4 network of the adapter carrying the default route. Ranges accept CIDR or start–end. Size policy: up to 1,024 addresses runs freely; 1,025 to 65,536 requires confirmation; more is refused. Network and broadcast addresses are skipped. 64 addresses probed concurrently. For addresses on the Technician's own subnet, ARP and ICMP run together and a Device exists if either answers; elsewhere ICMP only. Hostname comes from reverse lookup only. Vendor comes from a bundled copy of the IEEE OUI registry, matched on the MAC prefix. A Device with an ARP answer but no echo reply is flagged "no ping reply".
- **IP Info**: per adapter: name, up/down, IPv4/IPv6 with prefixes, gateway, DNS servers, MAC, DHCP enabled, lease obtained/expires. Active adapters first. The public lookup is an external call, allowed because IP Info's subject is the internet; Tools whose subject is not the internet make none (Speed Test, specified separately, is the other exception). Its failure does not block local details.
- **Subnet Calculator**: accepts `address/prefix` or address plus dotted mask; outputs network, broadcast, first/last usable, usable count, prefix, dotted mask, wildcard; a contains-check for a second address. /31 yields two usable addresses and no broadcast; /32 yields one. IPv6 outputs network, prefix, first/last address and address count (no broadcast or wildcard).

### Tauri shell

- One command per Tool to start a Run, returning its Run id; events are forwarded to the UI over a Tauri channel. One command stops a Run by id. Commands for Saved Target create/read/update/delete, import and export, and Copy as text.
- The shell constructs the real network adapter and passes the app data folder to the Saved Target store. It contains no Tool logic.

### UI

- Sidebar navigation listing the seven Tools plus Saved Targets. Tool state and live Runs live in an app-level store above the Tool screens, so switching Tools never resets or stops anything.
- Target inputs offer Saved Targets and an IPv4/IPv6 force toggle where relevant.
- Action menus on Devices, Hops and Saved Targets open the chosen Tool pre-filled and start the Run.
- Continuous Ping draws a live latency graph per Target with timeouts marked.
- Dark theme default with a light toggle; monospace for addresses, ports and Results.

## Testing Decisions

- **What a good test is**: it calls the toolbox core's public entry points (S1) with a request and asserts on the emitted events, the final Result and the Copy as text output. It never reaches into internal functions or data structures, so the internals can be refactored freely. Tests are written test-first for pure logic.
- **Default setup — S1 with fake S2**: most tests inject the scripted network adapter and controllable clock. This covers:
  - Subnet Calculator: table-driven cases for /0–/32, /31 and /32 edge cases, dotted-mask input, the contains-check, IPv6 prefixes, and invalid input.
  - Target validation and the Resolved Address policy: literal IPs, multi-address hostnames, forced IPv6, resolution failure.
  - Ping: per-echo statuses including fragmentation needed and unreachable; summary statistics; stop mid-Run.
  - Continuous Ping: loss, min/avg/max and jitter over scripted replies and timeouts using the fake clock; three concurrent Runs.
  - Traceroute: Hop assembly, `*` for silent probes, stopping at the Target, the 30-Hop limit, hostnames arriving after their Hop.
  - DNS Lookup: record shaping per type, automatic PTR for IP input, distinct NXDOMAIN/SERVFAIL/empty/no-response outcomes.
  - Port Check: list and range parsing, deduplication, the 1,024-port cap, Open/Closed/Filtered classification, Port Preset contents.
  - LAN Scan: range parsing, the size policy, own-subnet ARP-plus-ping versus ping-only elsewhere, "no ping reply" flagging, vendor lookup from MAC.
  - IP Info: adapter ordering and the public-lookup failure path.
  - Copy as text: golden-text comparisons for each Tool's Result, including stopped Runs.
  - Saved Target store: create/read/update/delete, persistence across store instances, and export/import round trip with the duplicate-skip rule, using a temporary folder.
- **Smoke tests — S1 with real S2**: a small set run against the real Windows network adapter to prove the API calls work: Ping to `127.0.0.1`, Traceroute to `127.0.0.1` (one Hop), Port Check against a TCP listener the test opens (Open) and an unused localhost port (Closed), adapter enumeration returning at least one adapter. Tests needing the internet (public lookup, DNS against public servers) are opt-in and not run by default.
- **UI**: no automated UI tests in v1; the UI is a thin shell over S1 and is verified manually.
- **Prior art**: none yet; this is a new repository. The `tdd` skill guides the test-first loop.

## Out of Scope

- Remote agents, a central server, or running any Tool from a machine other than the Technician's PC (ADR 0001).
- macOS and Linux builds.
- Anything that needs Administrator rights, including endpoint actions such as flushing DNS, renewing DHCP or resetting an adapter.
- MTR-style continuous Traceroute with per-Hop loss.
- Wake-on-LAN and a MAC field on Saved Targets.
- A persistent Run history log.
- A multi-Target monitor dashboard with alerts.
- Whois, VLSM subnet splitting, and offline utilities (password, hash, base64 and similar).
- Speed Test is not part of v1; it is specified separately in [.scratch/speed-test/spec.md](../speed-test/spec.md), which also relaxes the "only external call" rule below for Tools whose subject is the internet.
- UDP port checks and user-defined Port Presets.
- IPv6 LAN Scan, and hostname discovery beyond reverse DNS (NetBIOS, mDNS, LLMNR).
- Saved Target groups or tags.
- Team features: shared Saved Targets, code signing, auto-update.
- UI localisation.

## Further Notes

- **Decisions made while writing this spec** that were not explicitly discussed; review before implementation if any feel wrong:
  - Saved Target import skips entries whose label and address both match an existing one.
  - Port Check caps a Run at 1,024 ports and runs 32 connects in parallel.
  - LAN Scan hostnames come from reverse DNS only.
  - DNS Lookup queries servers directly, like `nslookup`, so the hosts file is not consulted.
  - Jitter is the mean absolute difference between consecutive successful round-trip times.
  - The default gateway is offered as a quick Target in Continuous Ping.
  - Active adapters are listed first in IP Info, inactive ones collapsed.
- Windows PCs drop ICMP echo by default, so LAN Scan will routinely report Devices flagged "no ping reply" on the Technician's own subnet. This is expected behaviour.
- LAN Scan sweeps may trigger alerts from endpoint security or intrusion detection on the network. The Technician accepts this.
- Suggested build order: Subnet Calculator (pure, establishes the core and test harness), Copy as text, Port Check, Ping and Continuous Ping, Traceroute, DNS Lookup, IP Info, LAN Scan, then Saved Targets and cross-Tool actions.
