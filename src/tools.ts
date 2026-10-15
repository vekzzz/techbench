import {
  Activity01Icon,
  Bookmark01Icon,
  Calculator01Icon,
  DashboardSpeed01Icon,
  InformationCircleIcon,
  Plug01Icon,
  Radar01Icon,
  Route01Icon,
  Search01Icon,
} from "@hugeicons/core-free-icons";

export type ToolId =
  | "ping"
  | "traceroute"
  | "dns-lookup"
  | "port-check"
  | "lan-scan"
  | "ip-info"
  | "speed-test"
  | "subnet-calculator";

/** Everything the sidebar can show: the Tools plus Saved Targets. */
export type ScreenId = ToolId | "saved-targets";

export type Screen = {
  id: ScreenId;
  name: string;
  summary: string;
  icon: typeof Activity01Icon;
};

export const TOOLS: Screen[] = [
  { id: "ping", name: "Ping", summary: "Reachability and round-trip time, once or continuously.", icon: Activity01Icon },
  { id: "traceroute", name: "Traceroute", summary: "Every router Hop between this PC and a Target.", icon: Route01Icon },
  { id: "dns-lookup", name: "DNS Lookup", summary: "Ask a DNS server for a Target's records.", icon: Search01Icon },
  { id: "port-check", name: "Port Check", summary: "Whether TCP ports on a Target accept connections.", icon: Plug01Icon },
  { id: "lan-scan", name: "LAN Scan", summary: "Discover the Devices present in an address range.", icon: Radar01Icon },
  { id: "ip-info", name: "IP Info", summary: "This PC's adapters and public address.", icon: InformationCircleIcon },
  { id: "speed-test", name: "Speed Test", summary: "Latency, download speed and how much latency grows under load.", icon: DashboardSpeed01Icon },
  { id: "subnet-calculator", name: "Subnet Calculator", summary: "Network, broadcast, masks and usable range.", icon: Calculator01Icon },
];

export const SAVED_TARGETS: Screen = {
  id: "saved-targets",
  name: "Saved Targets",
  summary: "Targets stored under a label, ready for any Tool.",
  icon: Bookmark01Icon,
};

export function screenById(id: ScreenId): Screen {
  return id === SAVED_TARGETS.id ? SAVED_TARGETS : TOOLS.find((tool) => tool.id === id)!;
}
