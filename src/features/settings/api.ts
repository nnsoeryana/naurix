import { invoke } from "@tauri-apps/api/core";

import { DEFAULT_APPEARANCE, type Appearance } from "../../themes/types";

export type CursorStyle = "block" | "bar" | "underline";
export type Renderer = "dom" | "webgl";

export type Settings = {
  general: { closeOnCleanExit: boolean; confirmCloseRunning: boolean; naurixEnabled: boolean; naurixAnimations: boolean; naurixReduced: boolean };
  appearance: Appearance;
  terminal: {
    fontFamily: string | null;
    fontSize: number;
    scrollback: number;
    cursorBlink: boolean;
    cursorStyle: CursorStyle;
    renderer: Renderer;
    loginShell: boolean;
    shellPath: string | null;
  };
};

/** Mirrors `naurix_core::settings` defaults (used until the real values load). */
export const DEFAULT_SETTINGS: Settings = {
  general: { closeOnCleanExit: true, confirmCloseRunning: true, naurixEnabled: true, naurixAnimations: true, naurixReduced: false },
  appearance: DEFAULT_APPEARANCE,
  terminal: { fontFamily: null, fontSize: 13, scrollback: 5000, cursorBlink: true, cursorStyle: "block", renderer: "dom", loginShell: true, shellPath: null },
};

export const SCROLLBACK_PRESETS = [1000, 5000, 10000, 50000];
export const FONT_MIN = 8;
export const FONT_MAX = 32;
export const SCROLLBACK_MIN = 500;
export const SCROLLBACK_MAX = 100000;

export type TrustedRepo = { key: string; path: string };
export type SecurityInfo = {
  version: string;
  arch: string;
  dataDir: string;
  keychainService: string;
  /** Name of the OS credential store (Keychain, Secret Service, Credential Manager). */
  secretStore: string;
  mcpServers: number;
  mcpApproved: number;
  skills: number;
  trustedRepos: TrustedRepo[];
};
export type PerfSnapshot = { pid: number; rssMb: number | null; sessions: number };

export const settingsApi = {
  get: () => invoke<Settings>("settings_get"),
  update: (settings: Settings) => invoke<Settings>("settings_update", { settings }),
  reset: () => invoke<Settings>("settings_reset"),
  security: () => invoke<SecurityInfo>("security_info"),
  untrustRepo: (key: string) => invoke<void>("security_untrust_repo", { key }),
  revokeMcpApprovals: () => invoke<number>("security_revoke_mcp_approvals"),
  perf: () => invoke<PerfSnapshot>("perf_snapshot"),
};

/** Deep merge of a partial settings patch (used by the benchmark mode and tests). */
export function mergeSettings(base: Settings, patch: { general?: Partial<Settings["general"]>; appearance?: Partial<Appearance>; terminal?: Partial<Settings["terminal"]> }): Settings {
  return { general: { ...base.general, ...patch.general }, appearance: { ...base.appearance, ...patch.appearance }, terminal: { ...base.terminal, ...patch.terminal } };
}
