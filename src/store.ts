import { create } from "zustand";

import * as toolbox from "@/lib/toolbox";
import type { ScreenId } from "@/tools";

/** A Port Check Run as the UI follows it, from start to Result. */
export type PortCheckRun = {
  /** Identifies this Run in the UI, so updates from an older Run are ignored. */
  key: number;
  /** The shell's id for stopping it; null until the shell has accepted the Run. */
  id: toolbox.RunId | null;
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

export const useAppStore = create<AppState>((set, get) => ({
  screen: "subnet-calculator",
  showScreen: (screen) => set({ screen }),

  subnetCalculator: { input: "", check: "" },
  updateSubnetCalculator: (patch) =>
    set((state) => ({ subnetCalculator: { ...state.subnetCalculator, ...patch } })),

  portCheck: { target: "", port: "", family: "auto", error: null, run: null },
  updatePortCheck: (patch) => set((state) => ({ portCheck: { ...state.portCheck, ...patch } })),

  startPortCheck: async () => {
    const { target, port, family, run: previous } = get().portCheck;
    if (previous?.id != null && !previous.result) void toolbox.stopRun(previous.id);

    const key = ++lastRunKey;
    const updateRun = (change: (run: PortCheckRun) => PortCheckRun) =>
      set((state) => {
        const run = state.portCheck.run;
        return run?.key === key ? { portCheck: { ...state.portCheck, run: change(run) } } : state;
      });

    set((state) => ({
      portCheck: { ...state.portCheck, error: null, run: { key, id: null, resolved: null, ports: [], result: null } },
    }));
    try {
      const id = await toolbox.startPortCheck({ target, port, family }, (update) => {
        switch (update.kind) {
          case "resolved":
            return updateRun((run) => ({ ...run, resolved: update.address }));
          case "port":
            return updateRun((run) => ({ ...run, ports: [...run.ports, update.port] }));
          case "finished":
            return updateRun((run) => ({ ...run, result: update.result }));
        }
      });
      updateRun((run) => ({ ...run, id }));
    } catch (reason) {
      set((state) =>
        state.portCheck.run?.key === key ? { portCheck: { ...state.portCheck, error: String(reason), run: null } } : state,
      );
    }
  },

  stopPortCheck: () => {
    const run = get().portCheck.run;
    if (run?.id != null && !run.result) void toolbox.stopRun(run.id);
  },
}));
