import { create } from "zustand";

import * as toolbox from "@/lib/toolbox";
import type { ScreenId } from "@/tools";

/** A Port Check Run as the UI follows it, from start to Result. */
export type PortCheckRun = {
  /** Identifies this Run in the UI, so updates from an older Run are ignored. */
  key: number;
  /** The shell's id for stopping it; null until the shell has accepted the Run. */
  id: toolbox.RunId | null;
  /** How many ports the Run will check; null until the shell has accepted the Run. */
  portsRequested: number | null;
  resolved: string | null;
  ports: toolbox.PortReport[];
  result: toolbox.PortCheckResult | null;
};

/**
 * App-level state. Tool state and live Runs live here, above the Tool screens,
 * so switching Tools in the sidebar never resets or stops anything.
 */
type AppState = {
  screen: ScreenId;
  showScreen: (screen: ScreenId) => void;

  subnetCalculator: { input: string; check: string };
  updateSubnetCalculator: (patch: Partial<AppState["subnetCalculator"]>) => void;

  portCheck: toolbox.PortCheckInput & { error: string | null; run: PortCheckRun | null };
  updatePortCheck: (patch: Partial<toolbox.PortCheckInput>) => void;
  startPortCheck: () => Promise<void>;
  stopPortCheck: () => void;
};

let lastRunKey = 0;

function stopIfRunning(run: PortCheckRun | null) {
  if (run?.id != null && !run.result) void toolbox.stopRun(run.id);
}

export const useAppStore = create<AppState>((set, get) => ({
  screen: "subnet-calculator",
  showScreen: (screen) => set({ screen }),

  subnetCalculator: { input: "", check: "" },
  updateSubnetCalculator: (patch) =>
    set((state) => ({ subnetCalculator: { ...state.subnetCalculator, ...patch } })),

  portCheck: { target: "", ports: "", family: "auto", error: null, run: null },
  updatePortCheck: (patch) => set((state) => ({ portCheck: { ...state.portCheck, ...patch } })),

  startPortCheck: async () => {
    const { target, ports, family, run: previous } = get().portCheck;
    stopIfRunning(previous);

    const key = ++lastRunKey;
    const updateRun = (change: (run: PortCheckRun) => PortCheckRun) =>
      set((state) => {
        const run = state.portCheck.run;
        return run?.key === key ? { portCheck: { ...state.portCheck, run: change(run) } } : state;
      });

    set((state) => ({
      portCheck: {
        ...state.portCheck,
        error: null,
        run: { key, id: null, portsRequested: null, resolved: null, ports: [], result: null },
      },
    }));
    try {
      const started = await toolbox.startPortCheck({ target, ports, family }, (update) => {
        switch (update.kind) {
          case "resolved":
            return updateRun((run) => ({ ...run, resolved: update.address }));
          case "port":
            return updateRun((run) => ({ ...run, ports: [...run.ports, update.port] }));
          case "finished":
            return updateRun((run) => ({ ...run, result: update.result }));
        }
      });
      updateRun((run) => ({ ...run, id: started.id, portsRequested: started.portsRequested }));
    } catch (reason) {
      set((state) =>
        state.portCheck.run?.key === key ? { portCheck: { ...state.portCheck, error: String(reason), run: null } } : state,
      );
    }
  },

  stopPortCheck: () => stopIfRunning(get().portCheck.run),
}));
