import { invoke } from "@tauri-apps/api/core";

/** Typed calls into the toolbox core through the Tauri shell. Failures reject with a message string. */

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
