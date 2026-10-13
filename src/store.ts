import { create } from "zustand";

import type { ScreenId } from "@/tools";

/**
 * App-level state. Tool state lives here, above the Tool screens,
 * so switching Tools in the sidebar never resets anything.
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
