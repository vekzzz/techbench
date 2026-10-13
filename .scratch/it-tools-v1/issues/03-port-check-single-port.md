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

- [ ] The Target input accepts a hostname, IPv4 address or IPv6 address; an invalid Target is rejected with a clear message before the Run starts.
- [ ] A hostname with several addresses uses its first IPv4 address unless IPv6 is forced; the Result always states the Resolved Address.
- [ ] A hostname that fails to resolve produces a resolution-failure Result, not Filtered.
- [ ] An accepted connection is Open (with connect time), a refused one is Closed, and a timeout (2 s) or unreachable error is Filtered.
- [ ] A Run can be stopped; a Run keeps going when the Technician switches Tools, and its Result is there on return.
- [ ] Copy as text starts with a header naming the Tool, Target, Resolved Address, local timestamp with UTC offset, and the source PC's hostname and IP, followed by aligned plain-text rows and a summary line, with no Markdown.
- [ ] Core tests drive the toolbox core with the fake adapter: Target validation, the Resolved Address policy, Open/Closed/Filtered classification, resolution failure, stopping a Run, and golden Copy as text.
- [ ] Smoke tests with the real adapter: a TCP listener the test opens on localhost reports Open; an unused localhost port reports Closed.
