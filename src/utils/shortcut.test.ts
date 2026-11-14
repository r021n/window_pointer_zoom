import { describe, expect, it } from "vitest";
import type { ShortcutConfig } from "../types/settings";
import {
  findDuplicateShortcutAction,
  formatKeyboardEventToAccelerator,
  normalizeAccelerator,
} from "./shortcut";

describe("shortcut utils", () => {
  it("formats modifier + key KeyboardEvent into Tauri accelerator", () => {
    expect(
      formatKeyboardEventToAccelerator({
        key: "z",
        code: "KeyZ",
        ctrlKey: true,
        altKey: true,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBe("Ctrl+Alt+Z");

    expect(
      formatKeyboardEventToAccelerator({
        key: "ArrowUp",
        code: "ArrowUp",
        ctrlKey: true,
        altKey: true,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBe("Ctrl+Alt+Up");

    expect(
      formatKeyboardEventToAccelerator({
        key: "!",
        code: "Digit1",
        ctrlKey: true,
        altKey: false,
        shiftKey: true,
        metaKey: false,
      }),
    ).toBe("Ctrl+Shift+1");

    expect(
      formatKeyboardEventToAccelerator({
        key: " ",
        code: "Space",
        ctrlKey: false,
        altKey: true,
        shiftKey: false,
        metaKey: true,
      }),
    ).toBe("Alt+Super+Space");
  });

  it("allows standalone function keys without modifiers", () => {
    expect(
      formatKeyboardEventToAccelerator({
        key: "F8",
        code: "F8",
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBe("F8");
  });

  it("ignores modifier-only, Escape, and unmodified non-function keys", () => {
    expect(
      formatKeyboardEventToAccelerator({
        key: "Control",
        code: "ControlLeft",
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBeNull();

    expect(
      formatKeyboardEventToAccelerator({
        key: "Escape",
        code: "Escape",
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBeNull();

    expect(
      formatKeyboardEventToAccelerator({
        key: "a",
        code: "KeyA",
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      }),
    ).toBeNull();
  });

  it("normalizes modifier aliases and ordering", () => {
    expect(normalizeAccelerator("alt+control+z")).toBe("Ctrl+Alt+Z");
    expect(normalizeAccelerator("Win+Shift+F10")).toBe("Shift+Super+F10");
  });

  it("detects duplicate shortcut bindings across different actions", () => {
    const config: ShortcutConfig = {
      toggle: "Ctrl+Alt+Z",
      zoomIn: "Ctrl+Alt+Up",
      zoomOut: "Ctrl+Alt+Down",
      reset: "Ctrl+Alt+0",
    };

    expect(findDuplicateShortcutAction(config, "zoomIn", "Alt+Ctrl+Z")).toBe(
      "toggle",
    );
    expect(findDuplicateShortcutAction(config, "toggle", "Ctrl+Alt+Z")).toBeNull();
    expect(findDuplicateShortcutAction(config, "reset", "Ctrl+Shift+R")).toBeNull();
  });
});
