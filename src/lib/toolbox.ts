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

/** One Device LAN Scan found present at an address. */
export type DeviceInfo = {
  address: string;
  /** From a reverse lookup, which arrives after the Device is first reported. */
  hostname: string | null;
  /** Only known for a Device on this PC's own subnet, where ARP reaches. */
  mac: string | null;
  /** From the MAC's registered prefix, so only known for a Device whose MAC is. */
  vendor: string | null;
  responseMs: number | null;
  /** It answered ARP but not the ping. */
  noPingReply: boolean;
  /** Whether ARP could reach it, which is why a MAC may be blank. */
  onOwnSubnet: boolean;
};

export type LanScanResult = {
  /** The Devices found, in numeric IP order. */
  devices: DeviceInfo[];
  addressesTotal: number;
  addressesProbed: number;
  ending: RunEnding;
  failureReason: string | null;
  /** The one-line conclusion, worded by the toolbox core. */
  summary: string;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type LanScanUpdate =
  | { kind: "started"; addresses: number }
  | { kind: "device"; device: DeviceInfo }
  | { kind: "hostname"; address: string; hostname: string }
  | { kind: "progress"; probed: number; total: number }
  | { kind: "finished"; result: LanScanResult };

/** `range` as typed: CIDR, start–end or one address. `confirmed`: the Technician agreed to a big range. */
export type LanScanInput = { range: string; confirmed: boolean };

/** Starts a LAN Scan Run. Rejects with a message, before any Run starts, when the range is invalid, too big, or big and unconfirmed. */
export function startLanScan(input: LanScanInput, onUpdate: (update: LanScanUpdate) => void): Promise<{ id: RunId; addresses: number }> {
  const channel = new Channel<LanScanUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_lan_scan", { input, onUpdate: channel });
}

/** How many addresses a range spans, and whether scanning it needs the Technician's confirmation. Rejects when the range is invalid or too big. */
export function lanRangeInfo(range: string): Promise<{ addresses: number; needsConfirmation: boolean }> {
  return invoke("lan_range_info", { range });
}

/** The network of the adapter carrying the default route, as CIDR; null when it has no IPv4 address. */
export function defaultLanRange(): Promise<string | null> {
  return invoke("default_lan_range");
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

/** A Target stored under a label. */
export type SavedTarget = { id: number; label: string; address: string; note: string | null };

/** As typed: an empty note means none. The toolbox core validates the label and address. */
export type SavedTargetInput = { label: string; address: string; note: string };

/** Every call rejects with a message the Technician can read, e.g. when the address is invalid. */
export function listSavedTargets(): Promise<SavedTarget[]> {
  return invoke("list_saved_targets");
}

export function createSavedTarget(input: SavedTargetInput): Promise<SavedTarget> {
  return invoke("create_saved_target", { input });
}

export function updateSavedTarget(id: number, input: SavedTargetInput): Promise<SavedTarget> {
  return invoke("update_saved_target", { id, input });
}

export function deleteSavedTarget(id: number): Promise<void> {
  return invoke("delete_saved_target", { id });
}

/** Writes every Saved Target to the JSON file at `path`. */
export function exportSavedTargets(path: string): Promise<void> {
  return invoke("export_saved_targets", { path });
}

/** Adds the Saved Targets in the JSON file at `path`, skipping any whose label and address both match one already stored. */
export function importSavedTargets(path: string): Promise<{ added: number; skipped: number }> {
  return invoke("import_saved_targets", { path });
}

export type SpeedPhase = "latency" | "download" | "upload";

/** Which way a transfer moves data. */
export type SpeedDirection = "download" | "upload";

export type SpeedTestServer = {
  provider: string;
  /** The provider's location code, such as HKG. */
  location: string;
  address: string | null;
};

export type SpeedTestEnding = "running" | "completed" | "stopped" | "noInternet" | "rateLimited" | "interrupted" | "unavailable";

/** One direction's figures: its speed, and how latency behaved while it ran. */
export type DirectionResult = {
  mbps: number | null;
  /** Whole percentage of the plan's speed, when the Technician gave one. */
  planPercent: number | null;
  loadedLatencyMs: number | null;
  increaseMs: number | null;
  verdict: "Low" | "Moderate" | "High" | null;
};

export type UseCaseRating = {
  useCase: string;
  /** 1 to 5; null when a figure it needs was not measured. */
  stars: number | null;
};

export type SpeedTestResult = {
  server: SpeedTestServer | null;
  /** This PC's name, with the address its internet traffic leaves from and the Adapter it used. */
  pc: string;
  public: string;
  publicFailed: boolean;
  latencyMedianMs: number | null;
  jitterMs: number | null;
  download: DirectionResult;
  upload: DirectionResult;
  ratings: UseCaseRating[];
  dataUsedBytes: number;
  ending: SpeedTestEnding;
  /** The one-line conclusion, worded by the toolbox core. */
  summary: string;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

/** The speed a transfer carried over one second of a phase. */
export type SpeedSample = { phase: SpeedPhase; atSeconds: number; mbps: number };

/** A latency probe sent while a transfer was saturating the line. */
export type LoadedProbe = { direction: SpeedDirection; atSeconds: number; latencyMs: number };

export type SpeedTestUpdate =
  | { kind: "phase"; phase: SpeedPhase }
  | { kind: "server"; server: SpeedTestServer }
  | { kind: "latency"; medianMs: number; jitterMs: number }
  | ({ kind: "loadedProbe" } & LoadedProbe)
  | ({ kind: "sample" } & SpeedSample)
  | { kind: "finished"; result: SpeedTestResult };

/** The plan speeds are Mbps as typed, blank for none; the toolbox core validates them. */
/** Who measures the line: Cloudflare's public endpoints, or Ookla's own speedtest.exe if installed. */
export type SpeedProvider = "cloudflare" | "ookla";

export type SpeedTestInput = {
  family: AddressFamily;
  provider: SpeedProvider;
  planDownload: string;
  planUpload: string;
};

/** Whether Ookla's speedtest.exe is installed, so it can be offered. */
export function ooklaInstalled(): Promise<boolean> {
  return invoke("ookla_installed");
}

/** Why starting needs confirming (a metered, roaming or over-limit connection); null to just start. */
export function speedTestCostWarning(): Promise<string | null> {
  return invoke("speed_test_cost_warning");
}

/** Starts a Speed Test Run. Rejects with a message, before any Run starts, when a plan speed is invalid. */
export function startSpeedTest(input: SpeedTestInput, onUpdate: (update: SpeedTestUpdate) => void): Promise<{ id: RunId }> {
  const channel = new Channel<SpeedTestUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_speed_test", { input, onUpdate: channel });
}
