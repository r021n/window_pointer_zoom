import { load, type Store } from "@tauri-apps/plugin-store";
import type {
  PersistedEngineSettings,
  ShortcutConfig,
  ThemeMode,
} from "../types/settings";

export const SETTINGS_STORE_FILE = "settings.json";
export const ENGINE_SETTINGS_KEY = "engineSettings";
export const SHORTCUTS_KEY = "shortcuts";
export const THEME_KEY = "theme";

function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

function getSettingsStore(): Promise<Store> {
  return load(SETTINGS_STORE_FILE, { autoSave: false });
}

/** Persists zoom limits and panning settings to `settings.json`. */
export async function saveEngineSettings(
  settings: PersistedEngineSettings,
): Promise<void> {
  const store = await getSettingsStore();
  await store.set(ENGINE_SETTINGS_KEY, settings);
  await store.save();
}

/** Persists active keyboard shortcut bindings to `settings.json`. */
export async function saveShortcuts(shortcuts: ShortcutConfig): Promise<void> {
  const store = await getSettingsStore();
  await store.set(SHORTCUTS_KEY, shortcuts);
  await store.save();
}

/** Loads UI theme preference from `settings.json`, defaulting to `"system"`. */
export async function loadTheme(): Promise<ThemeMode> {
  const store = await getSettingsStore();
  const value = await store.get<unknown>(THEME_KEY);
  return isThemeMode(value) ? value : "system";
}

/** Persists UI theme preference to `settings.json`. */
export async function saveTheme(theme: ThemeMode): Promise<void> {
  const store = await getSettingsStore();
  await store.set(THEME_KEY, theme);
  await store.save();
}
