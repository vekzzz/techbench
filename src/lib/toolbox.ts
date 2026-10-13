import { Channel, invoke } from "@tauri-apps/api/core";

/** Typed calls into the toolbox core through the Tauri shell. Failures reject with a message string. */

export type RunId = number;

/** Which IP version a Run may use for its Resolved Address. */
export type AddressFamily = "auto" | "ipv4" | "ipv6";

export type RunEnding = "completed" | "stopped" | "resolutionFailed";

export function stopRun(id: RunId): Promise<void> {
  return invoke("stop_run", { id });
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

/** One of this PC's network adapters. Times are already worded by the toolbox core. */
export type AdapterInfo = {
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
  /** Present when DHCP is enabled. */
  dhcp: { server: string | null; obtained: string | null; expires: string | null } | null;
};

/** The public address line, worded by the toolbox core, and whether the lookup failed. */
export type PublicInfo = { text: string; failed: boolean };

export type IpInfoResult = {
  /** The adapter carrying the default route first, then other active adapters, then inactive ones. */
  adapters: AdapterInfo[];
  defaultRoute: string | null;
  public: PublicInfo;
  /** The Result as plain text, ready to paste into a ticket. */
  text: string;
};

export type IpInfoUpdate =
  | { kind: "adapters"; adapters: AdapterInfo[]; defaultRoute: string | null }
  | { kind: "public"; public: PublicInfo }
  | { kind: "finished"; result: IpInfoResult };

/** Reads this PC's adapters, then its public address. */
export function startIpInfo(onUpdate: (update: IpInfoUpdate) => void): Promise<{ id: RunId }> {
  const channel = new Channel<IpInfoUpdate>();
  channel.onmessage = onUpdate;
  return invoke("start_ip_info", { onUpdate: channel });
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
