import { pingDefaults } from "@/lib/toolbox";
import {
  continuousPingSlots,
  useAppStore,
  useDnsLookup,
  usePing,
  usePortCheck,
  useSavedTargets,
  useTraceroute,
} from "@/store";

/** The Tools an address found in one Tool can be taken to. */
export type LeadAction = "ping" | "continuous-ping" | "traceroute" | "port-check" | "dns-lookup";

export const LEAD_ACTIONS: { id: LeadAction; label: string }[] = [
  { id: "ping", label: "Ping" },
  { id: "continuous-ping", label: "Continuous Ping" },
  { id: "traceroute", label: "Traceroute" },
  { id: "port-check", label: "Port Check" },
  { id: "dns-lookup", label: "DNS Lookup" },
];

/**
 * Takes `address` to another Tool: switches to it with the address filled in and the Run started,
 * the same as if the Technician had typed the address and pressed start. The IP version goes back
 * to Auto, since the address itself says which it is.
 */
export async function followLead(action: LeadAction, address: string) {
  const app = useAppStore.getState();
  switch (action) {
    case "ping": {
      // Ping's count, size and timeout are filled from the toolbox core's defaults when its screen first opens.
      const { input } = usePing.getState();
      if (input.count === "" && input.payloadSize === "" && input.timeoutMs === "") {
        usePing.getState().update(await pingDefaults());
      }
      usePing.getState().update({ target: address, family: "auto" });
      app.setPingMode("once");
      app.showScreen("ping");
      void usePing.getState().start();
      return;
    }
    case "continuous-ping": {
      app.setPingMode("continuous");
      app.showScreen("ping");
      const idle = continuousPingSlots
        .map((slot, index) => ({ index, state: slot.getState() }))
        .filter(({ state }) => state.run === null || state.run.result !== null);
      if (idle.length === 0) {
        app.showNotice("Three Continuous Pings are already running. Stop one first, then try again.");
        return;
      }
      // Prefer a slot with nothing typed in it; otherwise reuse one whose Run has ended.
      const chosen = idle.find(({ state }) => state.input.target.trim() === "") ?? idle[0];
      app.setContinuousTargets(Math.max(app.continuousTargets, chosen.index + 1));
      chosen.state.update({ target: address, family: "auto" });
      void chosen.state.start();
      return;
    }
    case "traceroute":
      useTraceroute.getState().update({ target: address, family: "auto" });
      app.showScreen("traceroute");
      void useTraceroute.getState().start();
      return;
    case "port-check":
      // The ports stay as they were; with none entered, the screen asks for some, as it would for a typed Target.
      usePortCheck.getState().update({ target: address, family: "auto" });
      app.showScreen("port-check");
      void usePortCheck.getState().start();
      return;
    case "dns-lookup":
      // An address is looked up as PTR by the toolbox core.
      useDnsLookup.getState().update({ query: address });
      app.showScreen("dns-lookup");
      void useDnsLookup.getState().start();
      return;
  }
}

/** Opens the Saved Targets screen with a new Saved Target ready to confirm, labelled with `label` when known. */
export function saveAsTarget(address: string, label?: string | null) {
  useSavedTargets.getState().setDraft({ label: label ?? "", address, note: "" });
  useAppStore.getState().showScreen("saved-targets");
}
