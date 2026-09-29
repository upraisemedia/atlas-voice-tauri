import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Mirrors `AppError` in src-tauri/src/error.rs. */
export interface AppError {
  kind: string;
  message: string;
}

export interface AppInfo {
  name: string;
  version: string;
}

/** Mirrors `dictation::Phase`. */
export type Phase =
  | { phase: "idle" }
  | { phase: "arming"; take: number }
  | { phase: "recording"; take: number }
  | { phase: "transcribing"; take: number }
  | { phase: "injecting"; take: number };

/** Mirrors `services::ModelStatus`. */
export type ModelStatus =
  | { status: "notInstalled" }
  | { status: "downloading"; downloaded: number; total: number }
  | { status: "verifying" }
  | { status: "loading" }
  | { status: "ready" }
  | { status: "error"; message: string };

export interface ModelSpec {
  id: string;
  name: string;
  description: string;
  fileName: string;
  sizeBytes: number;
  license: string;
}

export type HotkeyStatus = "waitingForAccessibility" | "active" | "failed";

export interface Status {
  phase: Phase;
  accessibility: boolean;
  hotkey: HotkeyStatus;
  model: ModelSpec;
  modelStatus: ModelStatus;
}

export interface Notice {
  kind: "error" | "info";
  message: string;
}

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" && value !== null && "kind" in value && "message" in value
  );
}

export function errorMessage(value: unknown): string {
  return isAppError(value) ? value.message : String(value);
}

export const commands = {
  appInfo: () => invoke<AppInfo>("app_info"),
  getStatus: () => invoke<Status>("get_status"),
  openAccessibilitySettings: () => invoke<void>("open_accessibility_settings"),
  downloadModel: () => invoke<void>("download_model"),
  cancelDownload: () => invoke<void>("cancel_download"),
  cancelDictation: () => invoke<void>("cancel_dictation"),
};

/** Event names emitted by Rust (see dictation/controller.rs and services.rs). */
export const events = {
  onPhase: (cb: (phase: Phase) => void): Promise<UnlistenFn> =>
    listen<Phase>("dictation-state", (e) => cb(e.payload)),
  onNotice: (cb: (notice: Notice) => void): Promise<UnlistenFn> =>
    listen<Notice>("dictation-notice", (e) => cb(e.payload)),
  onTranscript: (cb: (text: string) => void): Promise<UnlistenFn> =>
    listen<string>("dictation-transcript", (e) => cb(e.payload)),
  onModel: (cb: (status: ModelStatus) => void): Promise<UnlistenFn> =>
    listen<ModelStatus>("model-state", (e) => cb(e.payload)),
  onHotkey: (cb: (status: HotkeyStatus) => void): Promise<UnlistenFn> =>
    listen<HotkeyStatus>("hotkey-status", (e) => cb(e.payload)),
};

/** Subscribes to several events and returns one cleanup function for useEffect. */
export function subscribeAll(subscriptions: Promise<UnlistenFn>[]): () => void {
  return () => {
    for (const subscription of subscriptions) {
      subscription.then((unlisten) => unlisten());
    }
  };
}
