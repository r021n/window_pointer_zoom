import { useCallback, useEffect, useState } from "react";
import { loadTheme, saveTheme } from "../services/settings-store";
import type { ThemeMode } from "../types/settings";

const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";

function readSystemPrefersDark(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia(DARK_MEDIA_QUERY).matches;
}

export interface ThemeController {
  theme: ThemeMode;
  resolvedTheme: "light" | "dark";
  setTheme: (next: ThemeMode) => Promise<void>;
}


/**
 * Manages persisted light/dark/system theme preference and applies the `.dark`
 * class to `document.documentElement`.
 */
export function useTheme(): ThemeController {
  const [theme, setThemeState] = useState<ThemeMode>("system");
  const [systemPrefersDark, setSystemPrefersDark] = useState<boolean>(
    readSystemPrefersDark,
  );

  useEffect(() => {
    let cancelled = false;

    void loadTheme()
      .then((stored) => {
        if (!cancelled) {
          setThemeState(stored);
        }
      })
      .catch(() => {
        // Fallback silently to default `"system"` when store is unavailable.
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }

    const mediaQuery = window.matchMedia(DARK_MEDIA_QUERY);
    const handleChange = (event: MediaQueryListEvent) => {
      setSystemPrefersDark(event.matches);
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, []);

  const resolvedTheme: "light" | "dark" =
    theme === "system" ? (systemPrefersDark ? "dark" : "light") : theme;

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.style.colorScheme = resolvedTheme;
  }, [resolvedTheme]);

  const setTheme = useCallback(async (next: ThemeMode) => {
    setThemeState(next);
    try {
      await saveTheme(next);
    } catch {
      // Keep UI responsive even if disk write fails in non-Tauri environments.
    }
  }, []);

  return {
    theme,
    resolvedTheme,
    setTheme,
  };
}
