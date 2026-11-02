import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// the console (src-tauri/src/utils/console.rs): what the app did, kept by the backend. each window keeps a copy that
// catches up the first time it's used and follows the backend's events after that. the canvas overlay
// (src/components/ConsoleOverlay.tsx) and the console window (src/windows/Console.tsx) show it

export type Level = "error" | "warn" | "info" | "debug";
export type Source = "run" | "node" | "blender" | "app" | "rust" | "js";
// `time` in milliseconds since the unix epoch, `node` a node's path in `tab`, `ms` how long what it's about took
export type ConsoleEntry = { id: number; time: number; level: Level; source: Source; message: string; node: string | null; tab: string | null; ms: number | null };
// settings.json "console"
export type ConsoleSettings = { debug: boolean; overlay: boolean; inline_lines: number; fade_seconds: number };

export const LEVELS: Level[] = ["error", "warn", "info", "debug"];
export const SOURCES: Source[] = ["run", "node", "blender", "app", "rust", "js"];

// the backend keeps this many (CAPACITY in console.rs), so does each window
const CAPACITY = 5000;
const ENTRIES_EVENT = "console_entries";
const CLEARED_EVENT = "console_cleared";
// sent to the main window to select and frame a node, payload { tab, node }
const REVEAL_EVENT = "console-reveal";
// a node that wasn't found by then isn't looked for anymore
const REVEAL_MS = 3000;
const label = getCurrentWebviewWindow().label;

const listeners = new Set<() => void>();
const changed = () => listeners.forEach((listener) => listener());
const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
};

// MARK: - Entries

let entries: ConsoleEntry[] = [];
let following = false;

// takes entries the window doesn't have yet. events come in order after what it has, the backlog can overlap them
function take(next: ConsoleEntry[]) {
    const last = entries.length ? entries[entries.length - 1].id : 0;
    if (next.every((entry) => entry.id > last)) entries = [...entries, ...next];
    else {
        const byId = new Map([...entries, ...next].map((entry) => [entry.id, entry]));
        entries = [...byId.values()].sort((a, b) => a.id - b.id);
    }
    if (entries.length > CAPACITY) entries = entries.slice(-CAPACITY);
    changed();
}

// starts following the backend's entries, the events first so none come between the backlog and them
function follow() {
    if (following) return;
    following = true;
    listen<ConsoleEntry[]>(ENTRIES_EVENT, (event) => take(event.payload));
    listen(CLEARED_EVENT, () => {
        entries = [];
        changed();
    });
    invoke<ConsoleEntry[]>("console_entries")
        .then((backlog) => take(backlog ?? []))
        .catch(() => {});
}

// every entry, oldest first
export function useConsoleEntries(): ConsoleEntry[] {
    follow();
    return useSyncExternalStore(subscribe, () => entries);
}

// a warning or error the window showed (the status bar), the console gets it too
export function logToConsole(level: Level, message: string) {
    invoke("console_log", { level, source: "app", message }).catch(() => {});
}

// MARK: - Settings

let settings: ConsoleSettings = { debug: false, overlay: true, inline_lines: 5, fade_seconds: 10 };
const takeSettings = (all: any) => {
    settings = { ...settings, ...(all?.console ?? {}) };
    changed();
};
invoke("get_settings")
    .then(takeSettings)
    .catch(() => {});
listen("settings_changed", (event: any) => takeSettings(event.payload));

export function useConsoleSettings(): ConsoleSettings {
    return useSyncExternalStore(subscribe, () => settings);
}

// MARK: - JS Console

// an argument the way the console shows it
function format(arg: unknown): string {
    if (typeof arg === "string") return arg;
    if (arg instanceof Error) return `${arg.message}${arg.stack ? `\n${arg.stack}` : ""}`;
    try {
        return JSON.stringify(arg) ?? String(arg);
    } catch {
        return String(arg);
    }
}

// this window's console.log/info/warn/error/debug still print where they did, in debug mode they go to the console too.
// called once per window at startup
export function captureConsole() {
    const methods: [keyof Console, Level][] = [
        ["log", "info"],
        ["info", "info"],
        ["warn", "warn"],
        ["error", "error"],
        ["debug", "debug"],
    ];
    // sending one can't send another
    let sending = false;
    for (const [method, level] of methods) {
        const original = (console[method] as (...args: unknown[]) => void).bind(console);
        (console as any)[method] = (...args: unknown[]) => {
            original(...args);
            if (!settings.debug || sending) return;
            sending = true;
            const message = `${label}: ${args.map(format).join(" ")}`.slice(0, 4000);
            invoke("console_log", { level, source: "js", message }).catch(() => {});
            sending = false;
        };
    }
}

// MARK: - Reveal

// a node the main window is asked to select and frame, `seq` tells one ask from the next
export type Reveal = { tab: string; node: string; seq: number };
let reveal: Reveal | null = null;

// selects and frames a node in the main window, showing its tab and opening the groups it's in
export function revealNode(tab: string, node: string) {
    emitTo("main", REVEAL_EVENT, { tab, node }).catch(() => {});
}

if (label === "main") {
    listen<{ tab: string; node: string }>(REVEAL_EVENT, ({ payload }) => {
        const asked = { ...payload, seq: (reveal?.seq ?? 0) + 1 };
        reveal = asked;
        changed();
        invoke("switch_active_instance", { id: payload.tab }).catch(() => {});
        setTimeout(() => takeReveal(asked.seq), REVEAL_MS);
    });
}

// the node waiting to be selected and framed
export function usePendingReveal(): Reveal | null {
    return useSyncExternalStore(subscribe, () => reveal);
}

// the node was framed (or can't be), it isn't waiting anymore
export function takeReveal(seq: number) {
    if (reveal?.seq !== seq) return;
    reveal = null;
    changed();
}
