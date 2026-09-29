import { invoke } from "@tauri-apps/api/core";

/** Mirrors `AppError` in src-tauri/src/error.rs. */
export interface AppError {
  kind: string;
  message: string;
}

export interface AppInfo {
  name: string;
  version: string;
}

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" && value !== null && "kind" in value && "message" in value
  );
}

export const commands = {
  appInfo: () => invoke<AppInfo>("app_info"),
};
