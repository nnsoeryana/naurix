import { invoke } from "@tauri-apps/api/core";
import { sessionRegistry } from "./features/terminal/registry";
import { terminalApi } from "./features/terminal/api";
import { workspaceApi } from "./features/workspaces/api";
import { useStore } from "./state/store";
import { mergeSettings } from "./features/settings/api";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let started = false;

/** Benchmark mode: opens N terminals when NAURIX_BENCH_* is set (see bench_config in src-tauri). */
export async function runBenchMode() {
  if (started) return;
  const cfg = await invoke<{ workspace: string; terminals: number; command: string | null; settings: string | null } | null>("bench_config");
  if (!cfg) return;
  started = true;
  const w = await workspaceApi.create(cfg.workspace);
  await useStore.getState().init();
  useStore.getState().selectWorkspace(w.id);
  if (cfg.settings) {
    // e.g. NAURIX_BENCH_SETTINGS='{"terminal":{"renderer":"webgl"}}' to compare configurations
    await useStore.getState().updateSettings((s) => mergeSettings(s, JSON.parse(cfg.settings!)));
  }
  for (let i = 0; i < cfg.terminals; i++) {
    useStore.getState().newTab();
    await sleep(400);
  }
  if (cfg.command) {
    await sleep(2000);
    for (const sp of Object.values(useStore.getState().spaces)) {
      for (const t of sp.tabs) {
        const id = sessionRegistry.get(t.activePaneId);
        if (id !== undefined) await terminalApi.write(id, `${cfg.command}\r`);
      }
    }
  }
}
