# Techbench

A personal toolbox of network diagnostics that a Technician runs from their own Windows PC. Every Tool executes locally on that PC; nothing runs on remote machines.

## Language

### People

**Technician**:
The IT person operating Techbench from their own PC.
_Avoid_: User, admin, operator

### Targets

**Target**:
The hostname or IP address a Tool is aimed at.
_Avoid_: Host, destination, endpoint

**Saved Target**:
A Target the Technician has stored under a human label (e.g. "Core switch") so any Tool can pick it.
_Avoid_: Bookmark, favorite, saved host

**Resolved Address**:
The single IP address a Run actually used after resolving its Target; IPv4 preferred unless the Technician forces IPv6.
_Avoid_: Destination IP, real IP

### Tools

**Tool**:
One self-contained diagnostic or utility the Technician runs, such as Ping or Traceroute.
_Avoid_: Command, utility, feature, module

**Run**:
One execution of a Tool, usually against a Target, producing a Result. Tools that look at the Technician's own PC, such as IP Info, run without one.
_Avoid_: Job, session, test

**Result**:
What a Run reports back; always copyable as plain text for pasting into a ticket.
_Avoid_: Output, report, log

**Ping**:
A Tool that sends ICMP echo requests to a Target and reports reachability and round-trip time.

**Continuous Ping**:
A Ping that repeats against one to three Targets side by side until the Technician stops it, tracking latency, loss and jitter as it goes.
_Avoid_: Ping -t, monitor (reserved for a future multi-Target dashboard)

**Traceroute**:
A Tool that lists each router hop between the Technician's PC and a Target.
_Avoid_: Tracert, trace

**Hop**:
One router on the path a Traceroute reports, identified by its position, address and hostname.
_Avoid_: Node, step, router

**DNS Lookup**:
A Tool that asks a DNS server for a Target's records, or an address's name.
_Avoid_: nslookup, dig, resolve

**Port Check**:
A Tool that tests whether chosen TCP ports on a single Target accept connections.
_Avoid_: Port scan, telnet test

**Open**:
A Port Check outcome: the Target accepted the connection.

**Closed**:
A Port Check outcome: the Target actively refused the connection, so it is up but nothing is listening.
_Avoid_: Refused, down

**Filtered**:
A Port Check outcome: no answer arrived before the timeout, usually because a firewall dropped it.
_Avoid_: Timeout, blocked, stealth

**Port Preset**:
A named set of commonly checked ports (e.g. RDP, SMB, Web) offered by Port Check.
_Avoid_: Port profile, port group

**LAN Scan**:
A Tool that sweeps an address range to discover which Devices are present.
_Avoid_: Network scan, IP scan, sweep

**Device**:
Something LAN Scan found present at an address, by ARP or ping, described by its IP, hostname, MAC and vendor. A Device that answers ARP but not ping is still a Device.
_Avoid_: Host, node, client, machine

**Adapter**:
One of the Technician's PC's network connections (Ethernet, Wi-Fi, VPN or virtual), as Windows reports it.
_Avoid_: NIC, interface, connection, network card

**Default Route Adapter**:
The Adapter that the PC's internet traffic leaves through, as the routing table decides; its gateway, DNS server and subnet are the defaults other Tools offer.
_Avoid_: Primary adapter, main connection, active adapter

**IP Info**:
A Tool that shows the Technician's own local and public addresses and adapter details.
_Avoid_: My IP, ipconfig

**Subnet Calculator**:
A Tool that derives network, broadcast, mask and usable range from an address and prefix. It sends no traffic.
_Avoid_: CIDR calculator, IP calculator

**Speed Test**:
A Tool that measures the internet line from the Technician's PC: download speed, upload speed, latency and jitter, and how much latency grows under load. It runs without a Target. "Speed Test" names the Tool; one execution of it is still a Run.
_Avoid_: Speedtest, bandwidth test, "a test" (for a Run)

**Speed Test Server**:
The public server a Speed Test Run measured against, named by its location code (e.g. Cloudflare HKG) and the IP address used.
_Avoid_: Target (a Speed Test has none)

**Loaded Latency**:
Latency measured while a Speed Test is saturating the line in one direction, reported separately for download and upload.
_Avoid_: Latency under load, ping under load

**Rating**:
How many stars (1 to 5) a Speed Test gives the connection for one use case (Web Browsing, Streaming, Gaming, Video Calls): the lowest of the stars its figures earn. Not rated when a figure it needs was not measured.
_Avoid_: Score, grade

**Increase Under Load**:
Loaded Latency minus idle latency, for one direction. A Speed Test grades it Low (under 30 ms), Moderate (30–100 ms) or High (over 100 ms); a High increase is the bufferbloat signature.
_Avoid_: Bufferbloat score, grade
