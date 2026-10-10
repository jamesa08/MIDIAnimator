import { useSyncExternalStore } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// what the main window's status bar shows (src/components/StatusBar.tsx): the keys that do something where the mouse is,
// and the last thing that happened. other windows have no status bar of their own, they send theirs to the main window

const MAIN = "main";
const HINTS_EVENT = "status_hints";
const MESSAGE_EVENT = "status_message";
// windows that never take the mouse or keys
const SILENT = ["drag-ghost", "splash"];

const label = getCurrentWebviewWindow().label;
const isMain = label === MAIN;

// a key and the name of the command it runs, the key written the way the keymap writes it ("Shift+A")
export type Hint = { key: string; name: string };
// `seq` goes up with every message, each one fades in again even when it says the same as the last
export type Message = { text: string; seq: number };

let hints: Hint[] = [];
let message: Message | null = null;
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((listener) => listener());
const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
};

const takeHints = (next: Hint[]) => {
    hints = next;
    changed();
};
const takeMessage = (text: string) => {
    message = { text, seq: (message?.seq ?? 0) + 1 };
    changed();
};

if (isMain) {
    listen<Hint[]>(HINTS_EVENT, (event) => takeHints(event.payload));
    listen<string>(MESSAGE_EVENT, (event) => takeMessage(event.payload));
}

// the keys this window says do something now, the window that said so last is the one shown
export function setHints(next: Hint[]) {
    if (isMain) takeHints(next);
    else if (!SILENT.includes(label)) emitTo(MAIN, HINTS_EVENT, next);
}

// shows `text` until the next message
export function showStatus(text: string) {
    if (isMain) takeMessage(text);
    else emitTo(MAIN, MESSAGE_EVENT, text);
}

export function useStatusHints(): Hint[] {
    return useSyncExternalStore(subscribe, () => hints);
}

export function useStatusMessage(): Message | null {
    return useSyncExternalStore(subscribe, () => message);
}
