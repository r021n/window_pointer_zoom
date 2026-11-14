import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

/** Queries the OS autostart registration state directly from the plugin. */
export function isAutostartEnabled(): Promise<boolean> {
  return isEnabled();
}

/** Enables or disables Windows launch-on-boot registration. */
export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  if (enabled) {
    await enable();
  } else {
    await disable();
  }
}
