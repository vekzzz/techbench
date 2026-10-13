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
