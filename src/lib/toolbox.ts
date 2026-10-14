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

export type EchoStatus = "reply" | "timedOut" | "unreachable" | "fragmentationNeeded";

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
