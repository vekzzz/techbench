import { create } from "zustand";

import { createRunStore } from "@/lib/run-store";
import * as toolbox from "@/lib/toolbox";
import type { ScreenId } from "@/tools";

/**
 * App-level state. Tool state lives here and in the Tools' run stores below, above the Tool
 * screens, so switching Tools in the sidebar never resets or stops anything.
 */
type AppState = {
  screen: ScreenId;
  showScreen: (screen: ScreenId) => void;

  subnetCalculator: { input: string; check: string };
  updateSubnetCalculator: (patch: Partial<AppState["subnetCalculator"]>) => void;
};

export const useAppStore = create<AppState>((set) => ({
  screen: "subnet-calculator",
  showScreen: (screen) => set({ screen }),

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
  adapters: toolbox.AdapterInfo[] | null;
  defaultRoute: string | null;
  public: toolbox.PublicInfo | null;
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
