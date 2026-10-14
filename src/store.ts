import { create } from "zustand";

import { createRunStore } from "@/lib/run-store";
import * as toolbox from "@/lib/toolbox";
import type { ScreenId } from "@/tools";

/**
 * App-level state. Tool state lives here and in the Tools' run stores below, above the Tool
 * screens, so switching Tools in the sidebar never resets or stops anything.
 */
/** What LAN Scan orders its Devices by. */
export type DeviceSort = "ip" | "hostname" | "vendor";

type AppState = {
  screen: ScreenId;
  showScreen: (screen: ScreenId) => void;

  /** Whether Ping sends a set number of echoes or keeps going until stopped. */
  pingMode: "once" | "continuous";
  setPingMode: (mode: AppState["pingMode"]) => void;
  /** How many Continuous Ping Targets are shown side by side, from 1 to 3. */
  continuousTargets: number;
  setContinuousTargets: (count: number) => void;

  /** A message for the Technician that stays until dismissed, e.g. why a Cross-Tool action did not start. */
  notice: string | null;
  showNotice: (notice: string | null) => void;

  /** How LAN Scan lists its Devices. */
  lanScanView: { sort: DeviceSort; filter: string };
  updateLanScanView: (patch: Partial<AppState["lanScanView"]>) => void;

  subnetCalculator: { input: string; check: string };
  updateSubnetCalculator: (patch: Partial<AppState["subnetCalculator"]>) => void;
};

export const useAppStore = create<AppState>((set) => ({
  screen: "subnet-calculator",
  showScreen: (screen) => set({ screen }),

  pingMode: "once",
  setPingMode: (pingMode) => set({ pingMode }),
  continuousTargets: 1,
  setContinuousTargets: (count) => set({ continuousTargets: Math.min(3, Math.max(1, count)) }),

  notice: null,
  showNotice: (notice) => set({ notice }),

  lanScanView: { sort: "ip", filter: "" },
  updateLanScanView: (patch) => set((state) => ({ lanScanView: { ...state.lanScanView, ...patch } })),

  subnetCalculator: { input: "", check: "" },
  updateSubnetCalculator: (patch) =>
    set((state) => ({ subnetCalculator: { ...state.subnetCalculator, ...patch } })),
}));

/** A Port Check Run as the UI follows it, from start to Result. */
export type PortCheckRun = {
  key: number;
  id: toolbox.RunId | null;
  /** How many ports the Run will check; null until the shell has accepted the Run. */
  portsRequested: number | null;
  resolved: string | null;
  ports: toolbox.PortReport[];
  result: toolbox.PortCheckResult | null;
};

const portCheckInput: toolbox.PortCheckInput = { target: "", ports: "", family: "auto" };

export const usePortCheck = createRunStore({
  input: portCheckInput,
  begin: (key): PortCheckRun => ({ key, id: null, portsRequested: null, resolved: null, ports: [], result: null }),
  start: toolbox.startPortCheck,
  accepted: (run, started) => ({ ...run, id: started.id, portsRequested: started.portsRequested }),
  apply: (run, update: toolbox.PortCheckUpdate) => {
    switch (update.kind) {
      case "resolved":
        return { ...run, resolved: update.address };
      case "port":
        return { ...run, ports: [...run.ports, update.port] };
      case "finished":
        return { ...run, result: update.result };
    }
  },
});

/** A Ping Run as the UI follows it, from start to Result. */
export type PingRun = {
  key: number;
  id: toolbox.RunId | null;
  /** How many echoes the Run will send; null until the shell has accepted the Run. */
  echoesRequested: number | null;
  resolved: string | null;
  echoes: toolbox.EchoReport[];
  result: toolbox.PingResult | null;
};

// The count, size and timeout are filled from the toolbox core's defaults when Ping first opens.
const pingInput: toolbox.PingInput = {
  target: "",
  count: "",
  payloadSize: "",
  timeoutMs: "",
  dontFragment: false,
  family: "auto",
};

export const usePing = createRunStore({
  input: pingInput,
  begin: (key): PingRun => ({ key, id: null, echoesRequested: null, resolved: null, echoes: [], result: null }),
  start: toolbox.startPing,
  accepted: (run, started) => ({ ...run, id: started.id, echoesRequested: started.echoesRequested }),
  apply: (run, update: toolbox.PingUpdate) => {
    switch (update.kind) {
      case "resolved":
        return { ...run, resolved: update.address };
      case "echo":
        return { ...run, echoes: [...run.echoes, update.echo] };
      case "finished":
        return { ...run, result: update.result };
    }
  },
});

/** An IP Info Run as the UI follows it: local details first, then the public lookup. */
export type IpInfoRun = {
  key: number;
  id: toolbox.RunId | null;
  adapters: toolbox.Adapter[] | null;
  defaultRoute: string | null;
  public: toolbox.PublicIp | null;
  result: toolbox.IpInfoResult | null;
};

export const useIpInfo = createRunStore({
  input: {},
  begin: (key): IpInfoRun => ({ key, id: null, adapters: null, defaultRoute: null, public: null, result: null }),
  start: (_input, onUpdate: (update: toolbox.IpInfoUpdate) => void) => toolbox.startIpInfo(onUpdate),
  accepted: (run, started) => ({ ...run, id: started.id }),
  apply: (run, update) => {
    switch (update.kind) {
      case "adapters":
        return { ...run, adapters: update.adapters, defaultRoute: update.defaultRoute };
      case "public":
        return { ...run, public: update.public };
      case "finished":
        return {
          ...run,
          adapters: update.result.adapters,
          defaultRoute: update.result.defaultRoute,
          public: update.result.public,
          result: update.result,
        };
    }
  },
});

/** How many recent echoes a Continuous Ping graph shows. */
export const GRAPH_POINTS = 60;

/** A Continuous Ping Run as the UI follows it. */
export type ContinuousPingRun = {
  key: number;
  id: toolbox.RunId | null;
  resolved: string | null;
  /** The most recent echoes, oldest first: round trip in ms, or null when lost. */
  points: { sequence: number; roundTripMs: number | null }[];
  statistics: toolbox.PingStatistics | null;
  result: toolbox.ContinuousPingResult | null;
};

function createContinuousPingSlot() {
  const input: toolbox.ContinuousPingInput = { target: "", family: "auto" };
  return createRunStore({
    input,
    begin: (key): ContinuousPingRun => ({ key, id: null, resolved: null, points: [], statistics: null, result: null }),
    start: toolbox.startContinuousPing,
    accepted: (run, started) => ({ ...run, id: started.id }),
    apply: (run, update: toolbox.ContinuousPingUpdate) => {
      switch (update.kind) {
        case "resolved":
          return { ...run, resolved: update.address };
        case "echo": {
          const point = { sequence: update.echo.sequence, roundTripMs: update.echo.roundTripMs };
          return { ...run, points: [...run.points, point].slice(-GRAPH_POINTS), statistics: update.statistics };
        }
        case "finished":
          return { ...run, result: update.result };
      }
    },
  });
}

/** One store per Continuous Ping Target slot: up to three run side by side. */
export const continuousPingSlots = [createContinuousPingSlot(), createContinuousPingSlot(), createContinuousPingSlot()];

/** A Traceroute Run as the UI follows it: Hops appear as found, hostnames fill in later. */
export type TracerouteRun = {
  key: number;
  id: toolbox.RunId | null;
  resolved: string | null;
  hops: toolbox.HopInfo[];
  result: toolbox.TracerouteResult | null;
};

const tracerouteInput: toolbox.TracerouteInput = { target: "", family: "auto" };

export const useTraceroute = createRunStore({
  input: tracerouteInput,
  begin: (key): TracerouteRun => ({ key, id: null, resolved: null, hops: [], result: null }),
  start: toolbox.startTraceroute,
  accepted: (run, started) => ({ ...run, id: started.id }),
  apply: (run, update: toolbox.TracerouteUpdate) => {
    switch (update.kind) {
      case "resolved":
        return { ...run, resolved: update.address };
      case "hop":
        return { ...run, hops: [...run.hops, update.hop] };
      case "hostname":
        return {
          ...run,
          hops: run.hops.map((hop) => (hop.number === update.hop ? { ...hop, hostname: update.hostname } : hop)),
        };
      case "finished":
        return { ...run, result: update.result };
    }
  },
});

/** A DNS Lookup Run as the UI follows it: one question, one answer. */
export type DnsLookupRun = {
  key: number;
  id: toolbox.RunId | null;
  result: toolbox.DnsLookupResult | null;
};

const dnsLookupInput: toolbox.DnsLookupInput = { query: "", recordType: "A", server: "" };

export const useDnsLookup = createRunStore({
  input: dnsLookupInput,
  begin: (key): DnsLookupRun => ({ key, id: null, result: null }),
  start: toolbox.startDnsLookup,
  accepted: (run, started) => ({ ...run, id: started.id }),
  apply: (run, update: toolbox.DnsLookupUpdate) => ({ ...run, result: update.result }),
});

/** A LAN Scan Run as the UI follows it: Devices appear as found, with a count of addresses probed. */
export type LanScanRun = {
  key: number;
  id: toolbox.RunId | null;
  /** How many addresses the Run will probe; null until the shell has accepted the Run. */
  total: number | null;
  probed: number;
  devices: toolbox.DeviceInfo[];
  result: toolbox.LanScanResult | null;
};

const lanScanInput: toolbox.LanScanInput = { range: "", confirmed: false };

export const useLanScan = createRunStore({
  input: lanScanInput,
  begin: (key): LanScanRun => ({ key, id: null, total: null, probed: 0, devices: [], result: null }),
  start: toolbox.startLanScan,
  accepted: (run, started) => ({ ...run, id: started.id, total: started.addresses }),
  apply: (run, update: toolbox.LanScanUpdate) => {
    switch (update.kind) {
      case "started":
        return { ...run, total: update.addresses };
      case "device":
        return { ...run, devices: [...run.devices, update.device] };
      case "hostname":
        return {
          ...run,
          devices: run.devices.map((device) =>
            device.address === update.address ? { ...device, hostname: update.hostname } : device,
          ),
        };
      case "progress":
        return { ...run, probed: update.probed, total: update.total };
      case "finished":
        return { ...run, probed: update.result.addressesProbed, devices: update.result.devices, result: update.result };
    }
  },
});

/** The Technician's Saved Targets, loaded from the toolbox core and kept in step with every change. */
type SavedTargetsState = {
  targets: toolbox.SavedTarget[];
  /** Why the list could not be read, e.g. a damaged file. */
  error: string | null;
  loaded: boolean;
  load: () => Promise<void>;
  /** Each of these rejects with the core's message when the input is invalid. */
  create: (input: toolbox.SavedTargetInput) => Promise<void>;
  update: (id: number, input: toolbox.SavedTargetInput) => Promise<void>;
  remove: (id: number) => Promise<void>;
  /** A Saved Target waiting to be filled in on the Saved Targets screen (Save as Target from a Device or Hop). */
  draft: toolbox.SavedTargetInput | null;
  setDraft: (draft: toolbox.SavedTargetInput | null) => void;
};

export const useSavedTargets = create<SavedTargetsState>((set, get) => ({
  targets: [],
  error: null,
  loaded: false,
  load: async () => {
    try {
      set({ targets: await toolbox.listSavedTargets(), error: null, loaded: true });
    } catch (reason) {
      set({ error: String(reason), loaded: true });
    }
  },
  create: async (input) => {
    await toolbox.createSavedTarget(input);
    await get().load();
  },
  update: async (id, input) => {
    await toolbox.updateSavedTarget(id, input);
    await get().load();
  },
  remove: async (id) => {
    await toolbox.deleteSavedTarget(id);
    await get().load();
  },
  draft: null,
  setDraft: (draft) => set({ draft }),
}));
