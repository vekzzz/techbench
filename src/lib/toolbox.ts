import { Channel, invoke } from "@tauri-apps/api/core";

/** Typed calls into the toolbox core through the Tauri shell. Failures reject with a message string. */

export type RunId = number;

/** Which IP version a Run may use for its Resolved Address. */
export type AddressFamily = "auto" | "ipv4" | "ipv6";

export type RunEnding = "running" | "completed" | "stopped" | "resolutionFailed";

export function stopRun(id: RunId): Promise<void> {
  return invoke("stop_run", { id });
}

/** The Copy as text of a Run still going; null once it has finished, or before it has anything. */
export function runText(id: RunId): Promise<string | null> {
  return invoke("run_text", { id });
}

export type PortOutcome = "open" | "closed" | "filtered";

export type PortReport = {
  port: number;
  /** What usually listens on this port, when it is well known. */
  service: string | null;
  outcome: PortOutcome;
  connectTimeMs: number | null;
};

export type PortCheckResult = {
  resolved: string | null;
  /** The ports checked so far, in port order. */
  ports: PortReport[];
  portsRequested: number;
  ending: RunEnding;
  failureReason: string | null;
  /** The one-line conclusion, worded by the toolbox core. */
  summary: string;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type PortCheckUpdate =
  | { kind: "resolved"; address: string }
  | { kind: "port"; port: PortReport }
  | { kind: "finished"; result: PortCheckResult };

/** `ports` as typed: one port, a comma-separated list, or ranges such as 8000-8010. */
export type PortCheckInput = { target: string; ports: string; family: AddressFamily };

export type StartedPortCheck = { id: RunId; portsRequested: number };

/** Starts a Port Check Run. Rejects with a message, before any Run starts, when the input is invalid. */
export function startPortCheck(
  input: PortCheckInput,
  onUpdate: (update: PortCheckUpdate) => void,
): Promise<StartedPortCheck> {
  const channel = new Channel<PortCheckUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_port_check", { input, onUpdate: channel });
}

/** A named set of commonly checked ports. Port Presets are built in and not editable. */
export type PortPreset = { name: string; ports: number[] };

export function portPresets(): Promise<PortPreset[]> {
  return invoke("port_presets");
}

export type EchoStatus = "reply" | "timedOut" | "unreachable" | "timeExceeded" | "fragmentationNeeded";

export type EchoReport = {
  /** Numbered from 1, in the order the echoes were sent. */
  sequence: number;
  status: EchoStatus;
  roundTripMs: number | null;
  /** IPv6 replies carry no TTL. */
  ttl: number | null;
  /** Who reported the destination unreachable, when known. */
  from: string | null;
};

export type PingResult = {
  resolved: string | null;
  echoes: EchoReport[];
  ending: RunEnding;
  failureReason: string | null;
  /** The one-line conclusion, worded by the toolbox core. */
  summary: string;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type PingUpdate =
  | { kind: "resolved"; address: string }
  | { kind: "echo"; echo: EchoReport }
  | { kind: "finished"; result: PingResult };

/** As typed: the count, payload size (bytes) and timeout (ms) are validated by the toolbox core. */
export type PingInput = {
  target: string;
  count: string;
  payloadSize: string;
  timeoutMs: string;
  dontFragment: boolean;
  family: AddressFamily;
};

export type StartedPing = { id: RunId; echoesRequested: number };

/** The starting values of Ping's editable fields, as the toolbox core defines them. */
export function pingDefaults(): Promise<Pick<PingInput, "count" | "payloadSize" | "timeoutMs">> {
  return invoke("ping_defaults");
}

/** Starts a Ping Run. Rejects with a message, before any Run starts, when the input is invalid. */
export function startPing(input: PingInput, onUpdate: (update: PingUpdate) => void): Promise<StartedPing> {
  const channel = new Channel<PingUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_ping", { input, onUpdate: channel });
}

export type AdapterKind = "ethernet" | "wireless" | "loopback" | "tunnel" | "other";

/** One of this PC's network adapters, worded by the toolbox core. */
export type Adapter = {
  name: string;
  description: string;
  kind: AdapterKind;
  up: boolean;
  mac: string | null;
  /** Each as address/prefix. */
  ipv4: string[];
  ipv6: string[];
  gateways: string[];
  dnsServers: string[];
  /** "On, server …", "On" or "Off". */
  dhcp: string;
  leaseObtained: string | null;
  leaseExpires: string | null;
};

/** The public address line, worded by the toolbox core, and whether the lookup failed. */
export type PublicIp = { text: string; failed: boolean };

export type IpInfoResult = {
  /** This PC's name, with the address its internet traffic leaves from. */
  pc: string;
  /** The adapter carrying the default route first, then other active adapters, then inactive ones. */
  adapters: Adapter[];
  defaultRoute: string | null;
  public: PublicIp;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type IpInfoUpdate =
  | { kind: "adapters"; adapters: Adapter[]; defaultRoute: string | null }
  | { kind: "public"; public: PublicIp }
  | { kind: "finished"; result: IpInfoResult };

/** Reads this PC's adapters, then its public address. */
export function startIpInfo(onUpdate: (update: IpInfoUpdate) => void): Promise<{ id: RunId }> {
  const channel = new Channel<IpInfoUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_ip_info", { onUpdate: channel });
}

/** Running totals after an echo; times in milliseconds. */
export type PingStatistics = {
  sent: number;
  received: number;
  lossPercent: number;
  /** A whole percent, or "<1" when some echoes were lost but fewer than 1%. */
  lossLabel: string;
  minMs: number | null;
  avgMs: number | null;
  maxMs: number | null;
  jitterMs: number | null;
};

export type ContinuousPingResult = {
  ending: RunEnding;
  failureReason: string | null;
  /** The one-line conclusion, worded by the toolbox core. */
  summary: string;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type ContinuousPingUpdate =
  | { kind: "resolved"; address: string }
  | { kind: "echo"; echo: EchoReport; statistics: PingStatistics }
  | { kind: "finished"; result: ContinuousPingResult };

export type ContinuousPingInput = { target: string; family: AddressFamily };

/** Pings a Target every second until the Run is stopped. Rejects with a message when the input is invalid. */
export function startContinuousPing(
  input: ContinuousPingInput,
  onUpdate: (update: ContinuousPingUpdate) => void,
): Promise<{ id: RunId }> {
  const channel = new Channel<ContinuousPingUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_continuous_ping", { input, onUpdate: channel });
}

/** The default route's gateway, offered as a quick Target; null when there is none to offer. */
export function defaultGateway(): Promise<string | null> {
  return invoke("default_gateway");
}

export type ProbeInfo = {
  status: "answered" | "unreachable" | "silent" | "couldNotSend";
  roundTripMs: number | null;
  /** The router that answered, when it was not the Hop's own address. */
  from: string | null;
};

/** One router on the path. */
export type HopInfo = {
  number: number;
  /** The first address that answered; null when every probe was silent. */
  address: string | null;
  /** Filled in by a later update, once a reverse lookup finds a name. */
  hostname: string | null;
  probes: ProbeInfo[];
  reachedTarget: boolean;
};

export type TracerouteResult = {
  ending: RunEnding;
  failureReason: string | null;
  /** The one-line conclusion, worded by the toolbox core. */
  summary: string;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type TracerouteUpdate =
  | { kind: "resolved"; address: string }
  | { kind: "hop"; hop: HopInfo }
  | { kind: "hostname"; hop: number; hostname: string }
  | { kind: "finished"; result: TracerouteResult };

export type TracerouteInput = { target: string; family: AddressFamily };

/** Traces the path to a Target. Rejects with a message, before any Run starts, when the input is invalid. */
export function startTraceroute(
  input: TracerouteInput,
  onUpdate: (update: TracerouteUpdate) => void,
): Promise<{ id: RunId }> {
  const channel = new Channel<TracerouteUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_traceroute", { input, onUpdate: channel });
}

/** The record types DNS Lookup can ask for, in the order the screen offers them. */
export const DNS_RECORD_TYPES = ["A", "AAAA", "CNAME", "MX", "TXT", "NS", "PTR", "SOA"] as const;
export type DnsRecordType = (typeof DNS_RECORD_TYPES)[number];

export type DnsRecordInfo = { name: string; recordType: DnsRecordType; ttl: number; value: string };

export type DnsLookupResult = {
  /** The name actually asked for: the reverse name when the query was an IP address. */
  asked: string;
  recordType: DnsRecordType;
  automaticType: boolean;
  /** The server's address, with the name it was typed as when that was a hostname. */
  server: string | null;
  /** Only when a server really answered. */
  answeredMs: number | null;
  /** Whether `asked` is worth stating: it differs from what was typed. */
  showAsked: boolean;
  records: DnsRecordInfo[];
  /** True when there are no records to show: the summary says why. */
  failed: boolean;
  /** The one-line conclusion, worded by the toolbox core. */
  summary: string;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type DnsLookupUpdate = { kind: "finished"; result: DnsLookupResult };

/** `recordType` is ignored for an IP address, which is always looked up as PTR; an empty `server` means this PC's own. */
export type DnsLookupInput = { query: string; recordType: DnsRecordType; server: string };

/** Asks a DNS server. Rejects with a message, before any Run starts, when the input is invalid. */
export function startDnsLookup(input: DnsLookupInput, onUpdate: (update: DnsLookupUpdate) => void): Promise<{ id: RunId }> {
  const channel = new Channel<DnsLookupUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_dns_lookup", { input, onUpdate: channel });
}

/** This PC's own DNS server (the default route adapter's first); null when none is configured. */
export function defaultDnsServer(): Promise<string | null> {
  return invoke("default_dns_server");
}

export type SubnetReport = {
  network: string;
  prefix: number;
  broadcast: string | null;
  firstUsable: string;
  lastUsable: string;
  usableCount: string;
  mask: string | null;
  wildcard: string | null;
};

export function calculateSubnet(input: string): Promise<SubnetReport> {
  return invoke("calculate_subnet", { input });
}

export function subnetContains(input: string, address: string): Promise<boolean> {
  return invoke("subnet_contains", { input, address });
}
