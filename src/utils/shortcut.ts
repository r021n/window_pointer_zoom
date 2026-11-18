import type { ShortcutConfig } from "../types/settings";

const IGNORED_KEYS = new Set([
  "Control",
  "Alt",
  "AltGraph",
  "Shift",
  "Meta",
  "OS",
  "Super",
  "CapsLock",
  "NumLock",
  "ScrollLock",
  "Fn",
  "FnLock",
  "Dead",
  "Unidentified",
  "Process",
  "Escape",
]);

const CODE_KEY_MAP: Readonly<Record<string, string>> = {
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Equal: "Equal",
  Minus: "Minus",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  Backquote: "Backquote",
  NumpadAdd: "NumpadAdd",
  NumpadSubtract: "NumpadSubtract",
  NumpadMultiply: "NumpadMultiply",
  NumpadDivide: "NumpadDivide",
  NumpadDecimal: "NumpadDecimal",
  NumpadEnter: "NumpadEnter",
};

const KEY_FALLBACK_MAP: Readonly<Record<string, string>> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  "=": "Equal",
  "+": "Plus",
  "-": "Minus",
  "[": "BracketLeft",
  "]": "BracketRight",
  "\\": "Backslash",
  ";": "Semicolon",
  "'": "Quote",
  ",": "Comma",
  ".": "Period",
  "/": "Slash",
  "`": "Backquote",
};

const MODIFIER_ORDER: readonly string[] = ["Ctrl", "Alt", "Shift", "Super"];

function resolveKeyName(code: string, key: string): string | null {
  if (IGNORED_KEYS.has(key)) {
    return null;
  }

  if (code) {
    if (/^Key([A-Z])$/.test(code)) {
      return code.slice(3);
    }
    if (/^Digit([0-9])$/.test(code)) {
      return code.slice(5);
    }
    if (/^Numpad([0-9])$/.test(code)) {
      return code;
    }
    if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) {
      return code;
    }
    if (code in CODE_KEY_MAP) {
      return CODE_KEY_MAP[code];
    }
  }

  if (/^[a-zA-Z0-9]$/.test(key)) {
    return key.toUpperCase();
  }
  if (/^F([1-9]|1[0-9]|2[0-4])$/i.test(key)) {
    return key.toUpperCase();
  }
  if (key in KEY_FALLBACK_MAP) {
    return KEY_FALLBACK_MAP[key];
  }
  if (
    key === "Enter" ||
    key === "Tab" ||
    key === "Backspace" ||
    key === "Delete" ||
    key === "Insert" ||
    key === "Home" ||
    key === "End" ||
    key === "PageUp" ||
    key === "PageDown"
  ) {
    return key;
  }

  return null;
}

export type ShortcutEventInput = Pick<
  KeyboardEvent,
  "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey"
>;

/**
 * Converts a DOM `KeyboardEvent` into a Tauri global-shortcut accelerator string
 * (e.g. `Ctrl+Alt+Z`). Returns `null` for modifier-only presses, `Escape`,
 * unsupported keys, or bare non-function keys with no modifier held.
 */
export function formatKeyboardEventToAccelerator(
  event: ShortcutEventInput,
): string | null {
  const keyName = resolveKeyName(event.code ?? "", event.key);
  if (!keyName) {
    return null;
  }

  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  if (event.metaKey) modifiers.push("Super");

  const isFunctionKey = /^F([1-9]|1[0-9]|2[0-4])$/.test(keyName);
  if (modifiers.length === 0 && !isFunctionKey) {
    return null;
  }

  return [...modifiers, keyName].join("+");
}

function normalizeModifierToken(token: string): string | null {
  switch (token.toLowerCase()) {
    case "ctrl":
    case "control":
    case "commandorcontrol":
    case "cmdorctrl":
      return "Ctrl";
    case "alt":
    case "option":
      return "Alt";
    case "shift":
      return "Shift";
    case "super":
    case "meta":
    case "cmd":
    case "command":
    case "win":
      return "Super";
    default:
      return null;
  }
}

/** Normalizes an accelerator string so equivalent modifier orders compare equal. */
export function normalizeAccelerator(accelerator: string): string {
  const tokens = accelerator
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean);

  if (tokens.length === 0) {
    return "";
  }

  const mods = new Set<string>();
  const keys: string[] = [];

  for (const token of tokens) {
    const mod = normalizeModifierToken(token);
    if (mod) {
      mods.add(mod);
    } else {
      keys.push(token.toUpperCase());
    }
  }

  const orderedMods = MODIFIER_ORDER.filter((m) => mods.has(m));
  return [...orderedMods, ...keys].join("+");
}

export type ShortcutActionKey = keyof ShortcutConfig;

/**
 * Returns the conflicting action key if `candidate` duplicates another action's
 * accelerator in `config`, or `null` when unique.
 */
export function findDuplicateShortcutAction(
  config: ShortcutConfig,
  targetAction: ShortcutActionKey,
  candidate: string,
): ShortcutActionKey | null {
  const normalizedCandidate = normalizeAccelerator(candidate);
  if (!normalizedCandidate) {
    return null;
  }

  for (const action of Object.keys(config) as ShortcutActionKey[]) {
    if (action === targetAction) {
      continue;
    }
    const binding = config[action];
    if (binding && normalizeAccelerator(binding) === normalizedCandidate) {
      return action;
    }
  }

  return null;
}
