import { useEffect, useMemo, useRef, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type Hint, setHints } from "./status";

// every key in the app goes through here, one listener per window. the backend owns the keymap (src-tauri/src/ui/keybinds.rs):
// commands grouped by scope, the app, an area (the node editor) or a modal (grab, the add menu, dialogs). components only
// say what a command does (useKeymap, useModal), never which key runs it.
// a key goes to the modal on top if there is one, it takes every key and ignores the ones it has no command for. otherwise
// to the area under the mouse, then the app. keys typed into a text field stay there, and a menu item's own key is left to
// the menu, which sends it back as an app command (APP_COMMAND_EVENT in ui/menu.rs)

// true when the element takes typing, it keeps its own keys, undo/redo and clipboard
export function isTextField(element: Element | null): boolean {
    const el = element as HTMLElement | null;
    return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
}

// MARK: - Keymap

export type KeyCommand = { id: string; name: string; keys: string[]; menu: boolean; fixed: boolean; hold: boolean; native: string | null };
export type KeyScope = { id: string; name: string; kind: "app" | "area" | "modal"; commands: KeyCommand[] };
export type Keymap = { preset: string; presets: string[]; builtin: boolean; platform: "mac" | "other"; scopes: KeyScope[] };

let keymap: Keymap | null = null;
const keymapListeners = new Set<() => void>();
const setKeymap = (next: Keymap) => {
    keymap = next;
    keymapListeners.forEach((listener) => listener());
    refreshHints();
};
invoke<Keymap>("get_keymap").then(setKeymap);
listen<Keymap>("keymap_changed", (event) => setKeymap(event.payload));

const subscribeKeymap = (listener: () => void) => {
    keymapListeners.add(listener);
    return () => keymapListeners.delete(listener);
};

// the keymap, null until it's loaded
export function useKeymapData(): Keymap | null {
    return useSyncExternalStore(subscribeKeymap, () => keymap);
}

// the command in `scope` that `key` runs, holds are never run
function commandFor(scope: string | null, key: string): string | null {
    const commands = keymap?.scopes.find((s) => s.id === scope)?.commands ?? [];
    return commands.find((c) => !c.hold && c.keys.includes(key))?.id ?? null;
}

// a menu item handles this key itself
function isNative(key: string): boolean {
    return !!keymap?.scopes.some((scope) => scope.commands.some((c) => c.native === key));
}

// MARK: - Keys

// modifiers in the order they're written, like the mac menus (⌃⌥⇧⌘)
export const MODIFIERS = ["Ctrl", "Alt", "Shift", "Cmd"] as const;
export type Modifier = (typeof MODIFIERS)[number];

const MODIFIER_CODES: Record<string, Modifier> = { ShiftLeft: "Shift", ShiftRight: "Shift", ControlLeft: "Ctrl", ControlRight: "Ctrl", AltLeft: "Alt", AltRight: "Alt", MetaLeft: "Cmd", MetaRight: "Cmd" };
const CODE_NAMES: Record<string, string> = {
    Backquote: "`",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    NumpadEnter: "NumEnter",
    NumpadAdd: "NumAdd",
    NumpadSubtract: "NumSubtract",
    NumpadMultiply: "NumMultiply",
    NumpadDivide: "NumDivide",
    NumpadDecimal: "NumDecimal",
    NumpadEqual: "NumEqual",
    NumLock: "NumClear",
};
const NAMED_CODES = ["Escape", "Tab", "Enter", "Space", "Backspace", "Delete", "Home", "End", "PageUp", "PageDown"];
export const MOUSE_BUTTONS = ["LeftMouse", "MiddleMouse", "RightMouse"];

// the name a key is bound by, from where it is on the keyboard (event.code) so it's the same in every layout. null for
// a modifier on its own or a key that can't be bound
export function keyName(code: string): string | null {
    if (/^Key[A-Z]$/.test(code)) return code.slice(3);
    if (/^Digit\d$/.test(code)) return code.slice(5);
    if (/^Numpad\d$/.test(code)) return `Num${code.slice(6)}`;
    if (/^F\d+$/.test(code)) return code;
    if (code in CODE_NAMES) return CODE_NAMES[code];
    if (NAMED_CODES.includes(code)) return code;
    return null;
}

// the modifier a key is, null for any other key
export function modifierName(code: string): Modifier | null {
    return MODIFIER_CODES[code] ?? null;
}

// the event.code values of a bound key name, for what react flow wants (its box select key)
export function keyCodes(name: string): string[] {
    const modifier = Object.entries(MODIFIER_CODES).filter(([, m]) => m === name);
    if (modifier.length > 0) return modifier.map(([code]) => code);
    if (/^[A-Z]$/.test(name)) return [`Key${name}`];
    if (/^\d$/.test(name)) return [`Digit${name}`];
    if (/^Num\d$/.test(name)) return [`Numpad${name.slice(3)}`];
    const named = Object.entries(CODE_NAMES).find(([, n]) => n === name);
    return [named ? named[0] : name];
}

type ModifierState = { ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean };

// a key with the modifiers held, written the way the keymap writes them ("Ctrl+Shift+A")
export function combo(modifiers: ModifierState | Modifier[], key: string): string {
    const held = Array.isArray(modifiers) ? modifiers : MODIFIERS.filter((m) => ({ Ctrl: modifiers.ctrlKey, Alt: modifiers.altKey, Shift: modifiers.shiftKey, Cmd: modifiers.metaKey })[m]);
    return [...MODIFIERS.filter((m) => held.includes(m)), key].join("+");
}

// a key's modifiers and its key name
export function splitCombo(key: string): { modifiers: Modifier[]; name: string } {
    const parts = key.split("+");
    return { modifiers: parts.slice(0, -1) as Modifier[], name: parts[parts.length - 1] };
}

const MAC_MODIFIERS: Record<Modifier, string> = { Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Cmd: "⌘" };
const OTHER_MODIFIERS: Record<Modifier, string> = { Ctrl: "Ctrl", Alt: "Alt", Shift: "Shift", Cmd: "Win" };
const MAC_NAMES: Record<string, string> = { Escape: "⎋", Tab: "⇥", Enter: "↩", Backspace: "⌫", Delete: "⌦", Up: "↑", Down: "↓", Left: "←", Right: "→", Home: "↖", End: "↘", PageUp: "⇞", PageDown: "⇟", NumEnter: "⌤" };
const NAMES: Record<string, string> = { LeftMouse: "Left Mouse", MiddleMouse: "Middle Mouse", RightMouse: "Right Mouse", NumAdd: "Num +", NumSubtract: "Num -", NumMultiply: "Num *", NumDivide: "Num /", NumDecimal: "Num .", NumEqual: "Num =", NumClear: "Clear", NumEnter: "Num Enter", PageUp: "Page Up", PageDown: "Page Down" };

// how a key is shown: "⇧⌘S" on a mac, "Ctrl+Shift+S" elsewhere
export function formatCombo(key: string, platform: Keymap["platform"]): string {
    const { modifiers, name } = splitCombo(key);
    const mac = platform === "mac";
    // a hold can be a modifier on its own
    const modifier = MODIFIERS.includes(name as Modifier) ? (mac ? MAC_MODIFIERS : OTHER_MODIFIERS)[name as Modifier] : null;
    const shown = modifier || (mac && MAC_NAMES[name]) || NAMES[name] || name.replace(/^Num(\d)$/, "Num $1");
    if (mac) return modifiers.map((m) => MAC_MODIFIERS[m]).join("") + shown;
    return [...modifiers.map((m) => OTHER_MODIFIERS[m]), shown].join("+");
}

// MARK: - Handlers

export type Handlers = Record<string, () => void>;

// what components say the commands of a scope do, the last one registered that's enabled runs them
type Registration = { scope: string; handlers: () => Handlers; enabled: () => boolean };
const registrations: Registration[] = [];

// what `command` in `scope` runs, none when nothing enabled handles it
function handlerFor(scope: string, command: string): (() => void) | null {
    for (let i = registrations.length - 1; i >= 0; i--) {
        const registration = registrations[i];
        if (registration.scope !== scope || !registration.enabled()) continue;
        const handler = registration.handlers()[command];
        if (handler) return handler;
    }
    return null;
}

// says what commands of `scope` do, until the returned function is called
export function registerKeymap(scope: string, handlers: Handlers | (() => Handlers), enabled: () => boolean = () => true): () => void {
    const registration = { scope, handlers: typeof handlers === "function" ? handlers : () => handlers, enabled };
    registrations.push(registration);
    refreshHints();
    return () => {
        const index = registrations.indexOf(registration);
        if (index >= 0) registrations.splice(index, 1);
        refreshHints();
    };
}

// what the commands of `scope` do while this component is mounted. `enabled` is read when a key is pressed
export function useKeymap(scope: string, handlers: Handlers, enabled?: () => boolean) {
    const handlersRef = useRef(handlers);
    handlersRef.current = handlers;
    const enabledRef = useRef(enabled);
    enabledRef.current = enabled;
    useEffect(
        () =>
            registerKeymap(
                scope,
                () => handlersRef.current,
                () => enabledRef.current?.() ?? true
            ),
        [scope]
    );
}

// an app command, by this window's handler or the backend's (run_app_command in ui/keybinds.rs)
function runApp(command: string) {
    const handler = handlerFor("app", command);
    if (handler) handler();
    else invoke("run_app_command", { id: command });
}

// MARK: - Modals

type ModalOptions = {
    // keys it has no command for still reach the page (typing in the add menu's search, tabbing through a dialog's
    // buttons), they just don't run any other command
    passthrough?: boolean;
    // takes every key and mouse press before anything else, returning whether it used it (the shortcut editor recording one)
    raw?: (event: KeyboardEvent | PointerEvent) => boolean;
};
// `scope` null takes every key and runs nothing (a drag react flow is doing)
type Modal = ModalOptions & { scope: string | null; handlers: () => Handlers };
const modals: Modal[] = [];

// takes every key until the returned function is called. the newest modal is the one keys go to
export function pushModal(scope: string | null, handlers: Handlers | (() => Handlers) = {}, options: ModalOptions = {}): () => void {
    const modal: Modal = { ...options, scope, handlers: typeof handlers === "function" ? handlers : () => handlers };
    modals.push(modal);
    refreshHints();
    return () => {
        const index = modals.indexOf(modal);
        if (index >= 0) modals.splice(index, 1);
        refreshHints();
    };
}

// a modal while `active`, its handlers can change between renders
export function useModal(scope: string | null, handlers: Handlers, active: boolean, options: ModalOptions = {}) {
    const handlersRef = useRef(handlers);
    handlersRef.current = handlers;
    const optionsRef = useRef(options);
    optionsRef.current = options;
    useEffect(() => {
        if (!active) return;
        const { passthrough } = optionsRef.current;
        return pushModal(scope, () => handlersRef.current, { passthrough, raw: optionsRef.current.raw && ((event) => optionsRef.current.raw!(event)) });
    }, [scope, active]);
}

// a drag that takes every key until the mouse button is let go (react flow's box select, link drag, node drag)
export function blockUntilRelease() {
    const pop = pushModal(null);
    const end = () => {
        pop();
        window.removeEventListener("pointerup", end, true);
        window.removeEventListener("pointercancel", end, true);
        window.removeEventListener("blur", end);
    };
    window.addEventListener("pointerup", end, true);
    window.addEventListener("pointercancel", end, true);
    window.addEventListener("blur", end);
}

// MARK: - Holds

// keys held down by name (modifiers too), for holds like pan
const held = new Set<string>();
const holdListeners = new Set<() => void>();
const setHeld = (name: string, down: boolean) => {
    if (held.has(name) === down) return;
    if (down) held.add(name);
    else held.delete(name);
    holdListeners.forEach((listener) => listener());
    refreshHints();
};

const subscribeHolds = (listener: () => void) => {
    holdListeners.add(listener);
    return () => holdListeners.delete(listener);
};

// true while a key of the hold command `command` in `scope` is held
export function useHold(scope: string, command: string): boolean {
    const keys =
        useKeymapData()
            ?.scopes.find((s) => s.id === scope)
            ?.commands.find((c) => c.id === command)?.keys ?? [];
    return useSyncExternalStore(subscribeHolds, () => keys.some((key) => held.has(key)));
}

// the event.code values of the hold command `command` in `scope`, for react flow's own key props
export function useHoldCodes(scope: string, command: string): string[] {
    const keys =
        useKeymapData()
            ?.scopes.find((s) => s.id === scope)
            ?.commands.find((c) => c.id === command)?.keys ?? [];
    // the same array until the keys change, react flow listens again whenever it gets a new one
    const codes = keys.flatMap(keyCodes).join(" ");
    return useMemo(() => (codes ? codes.split(" ") : []), [codes]);
}

// MARK: - Dispatch

// where the mouse was last, the area under it gets the keys
let mouse: { x: number; y: number } | null = null;

// the areas under the mouse, innermost first. before the mouse has been over the window, the window's first area (the
// node editor in the main window)
function areasUnderMouse(): string[] {
    const element = mouse ? document.elementFromPoint(mouse.x, mouse.y) : document.querySelector("[data-keymap-area]");
    const areas: string[] = [];
    let area = element?.closest<HTMLElement>("[data-keymap-area]");
    while (area) {
        areas.push(area.dataset.keymapArea!);
        area = area.parentElement?.closest<HTMLElement>("[data-keymap-area]");
    }
    return areas;
}

// MARK: - Status

// the keys that do something now, for the status bar: the modal's on top, or the area under the mouse's. holding
// modifiers shows what they do with the area's and the app's keys
function currentHints(): Hint[] {
    const commands = (scope: string | null) => keymap?.scopes.find((s) => s.id === scope)?.commands ?? [];
    const modal = modals[modals.length - 1];
    if (modal) {
        if (modal.raw) return [];
        const handlers = modal.handlers();
        return commands(modal.scope)
            .filter((c) => handlers[c.id] && c.keys.length)
            .map((c) => ({ key: c.keys[0], name: c.name }));
    }
    const modifiers = MODIFIERS.filter((m) => held.has(m));
    const same = (key: string) => {
        const { modifiers: of } = splitCombo(key);
        return of.length === modifiers.length && of.every((m) => modifiers.includes(m));
    };
    const hints: Hint[] = [];
    for (const scope of [...areasUnderMouse(), ...(modifiers.length ? ["app"] : [])]) {
        for (const command of commands(scope)) {
            // an area's command shows while something here runs it, holds are read where they're used
            if (scope !== "app" && !command.hold && !handlerFor(scope, command.id)) continue;
            const key = command.keys.find(same);
            if (key && !hints.some((h) => h.key === key)) hints.push({ key, name: command.name });
        }
    }
    return hints;
}

// the mouse is over this window, set when it comes in from outside and cleared when it leaves
let mouseInside = false;
let shownHints = "";
// sends the hints when they changed, or always when `force` (this window took the mouse back from another one). only the
// window with the mouse or the keys sends them, one in the background would take the status bar from it
function refreshHints(force = false) {
    if (!force && !mouseInside && !document.hasFocus()) return;
    const hints = currentHints();
    const key = JSON.stringify(hints);
    if (key === shownHints && !force) return;
    shownHints = key;
    setHints(hints);
}

let shownAreas = "";
function refreshAreas() {
    const areas = areasUnderMouse().join(" ");
    if (areas === shownAreas) return;
    shownAreas = areas;
    refreshHints();
}

const stop = (event: Event) => {
    event.preventDefault();
    event.stopPropagation();
};

function handleKeyDown(event: KeyboardEvent) {
    if (event.isComposing) return;
    const modifier = modifierName(event.code);
    const name = keyName(event.code);
    if (modifier || name) setHeld(modifier ?? name!, true);

    const modal = modals[modals.length - 1];
    if (modal?.raw) {
        if (modal.raw(event)) stop(event);
        return;
    }
    // a modifier on its own goes through, nothing is bound to one (holds read it above)
    if (!name) return;
    const key = combo(event, name);

    if (modal) {
        const command = commandFor(modal.scope, key);
        const handler = command && modal.handlers()[command];
        if (handler) {
            stop(event);
            if (!event.repeat) handler();
        } else if (!modal.passthrough) {
            stop(event);
        }
        return;
    }

    if (isTextField(event.target as Element)) return;
    if (isNative(key)) return;

    for (const scope of [...areasUnderMouse(), "app"]) {
        const command = commandFor(scope, key);
        if (!command) continue;
        const handler = handlerFor(scope, command);
        if (!handler && scope !== "app") continue;
        stop(event);
        if (event.repeat) return;
        if (handler) handler();
        else runApp(command);
        return;
    }
}

function handleKeyUp(event: KeyboardEvent) {
    const name = modifierName(event.code) ?? keyName(event.code);
    if (name) setHeld(name, false);
    // macos sends no keyup for a key let go while cmd is held
    if (modifierName(event.code) === "Cmd") [...held].filter((key) => !MODIFIERS.includes(key as Modifier)).forEach((key) => setHeld(key, false));
}

// set by a mouse press a modal used, the rest of that click (mousedown, mouseup, click, the context menu) is swallowed so
// nothing under the cursor reacts to it, like blender's confirming click
let swallowClick = false;

function handlePointerDown(event: PointerEvent) {
    swallowClick = false;
    mouse = { x: event.clientX, y: event.clientY };
    const modal = modals[modals.length - 1];
    if (!modal) return;
    if (modal.raw) {
        if (modal.raw(event)) {
            stop(event);
            swallowClick = true;
        }
        return;
    }
    const button = MOUSE_BUTTONS[event.button];
    const command = button && commandFor(modal.scope, combo(event, button));
    const handler = command && modal.handlers()[command];
    if (!handler) return;
    event.stopPropagation();
    swallowClick = true;
    handler();
}

function swallow(event: MouseEvent) {
    if (!swallowClick) return;
    stop(event);
    if (event.type === "click" || event.type === "auxclick") swallowClick = false;
}

window.addEventListener("keydown", handleKeyDown, true);
window.addEventListener("keyup", handleKeyUp, true);
window.addEventListener("pointerdown", handlePointerDown, true);
for (const type of ["mousedown", "mouseup", "click", "auxclick", "contextmenu"]) window.addEventListener(type, swallow as EventListener, true);
window.addEventListener(
    "mousemove",
    (event) => {
        mouse = { x: event.clientX, y: event.clientY };
        refreshAreas();
    },
    true
);
window.addEventListener("blur", () => [...held].forEach((key) => setHeld(key, false)));
// the mouse came in from outside the window, or the window was focused: its hints are the ones shown again
window.addEventListener(
    "mouseover",
    (event) => {
        if (event.relatedTarget) return;
        mouseInside = true;
        refreshHints(true);
    },
    true
);
window.addEventListener("mouseout", (event) => !event.relatedTarget && (mouseInside = false), true);
window.addEventListener("focus", () => refreshHints(true));

// a menu item picked or its shortcut pressed in this window. a modal ignores it like any other key, except quit so a modal
// that never ended can't keep the app open (not while the shortcut editor is recording a key, that's the key it wants)
getCurrentWebviewWindow().listen<string>("app-command", ({ payload }) => {
    const modal = modals[modals.length - 1];
    if (modal && (payload !== "quit" || modal.raw)) return;
    runApp(payload);
});
