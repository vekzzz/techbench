import { create } from "zustand";

import { stopRun, type RunId } from "@/lib/toolbox";

/** What every Run the UI follows has in common. */
export type FollowedRun = {
  /** Identifies this Run in the UI, so updates from an older Run are ignored. */
  key: number;
  /** The shell's id for stopping it; null until the shell has accepted the Run. */
  id: RunId | null;
  /** The Result, null until the Run has finished. */
  result: unknown;
};

export type RunStore<Input, Run> = {
  input: Input;
  /** Why the shell rejected the input, before any Run started. */
  error: string | null;
  run: Run | null;
  update: (patch: Partial<Input>) => void;
  start: () => Promise<void>;
  stop: () => void;
  /** Stops the Run and returns the store to its starting input with no Run. */
  clear: () => void;
};

/**
 * A store for one Tool's input and its latest Run. It lives above the Tool screens, so switching
 * Tools never resets or stops anything. Starting a new Run stops the one still going.
 */
export function createRunStore<Input, Run extends FollowedRun, Update, Started extends { id: RunId }>(tool: {
  input: Input;
  /** The Run as it looks the moment it starts. */
  begin: (key: number) => Run;
  start: (input: Input, onUpdate: (update: Update) => void) => Promise<Started>;
  /** Records what the shell said when it accepted the Run. */
  accepted: (run: Run, started: Started) => Run;
  /** Applies one update from the Run's channel. */
  apply: (run: Run, update: Update) => Run;
}) {
  let lastKey = 0;
  const stopIfRunning = (run: Run | null) => {
    if (run?.id != null && run.result === null) void stopRun(run.id);
  };

  return create<RunStore<Input, Run>>((set, get) => {
    const changeRun = (key: number, change: (run: Run) => Run) =>
      set((state) => (state.run?.key === key ? { run: change(state.run) } : state));

    return {
      input: tool.input,
      error: null,
      run: null,
      update: (patch) => set((state) => ({ input: { ...state.input, ...patch } })),
      start: async () => {
        const { input, run: previous } = get();
        stopIfRunning(previous);
        const key = ++lastKey;
        set({ error: null, run: tool.begin(key) });
        try {
          const started = await tool.start(input, (update) => changeRun(key, (run) => tool.apply(run, update)));
          changeRun(key, (run) => tool.accepted(run, started));
        } catch (reason) {
          set((state) => (state.run?.key === key ? { error: String(reason), run: null } : state));
        }
      },
      stop: () => stopIfRunning(get().run),
      clear: () => {
        stopIfRunning(get().run);
        set({ input: tool.input, error: null, run: null });
      },
    };
  });
}
