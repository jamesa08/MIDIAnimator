import { useSyncExternalStore } from "react";
import { useModal } from "../utils/keymap";

interface UnsavedChangesModalProps {
    onSave: () => void;
    onDiscard: () => void;
    onCancel: () => void;
}

export type UnsavedAnswer = "save" | "discard" | "cancel";

// the question askUnsaved is waiting on, UnsavedChangesPrompt shows it
let waiting: ((answer: UnsavedAnswer) => void) | null = null;
const listeners = new Set<() => void>();
const changed = () => listeners.forEach((listener) => listener());
const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
};

// asks whether to save the tab on screen's unsaved changes before it's closed (the window closing, a file opened again),
// resolves to what was picked. one still waiting is cancelled
export function askUnsaved(): Promise<UnsavedAnswer> {
    waiting?.("cancel");
    return new Promise((resolve) => {
        waiting = resolve;
        changed();
    });
}

const answer = (picked: UnsavedAnswer) => {
    const resolve = waiting;
    waiting = null;
    changed();
    resolve?.(picked);
};

// the question askUnsaved asked, while it waits. App has it
export function UnsavedChangesPrompt() {
    const asking = useSyncExternalStore(subscribe, () => waiting !== null);
    return asking ? <UnsavedChangesModal onSave={() => answer("save")} onDiscard={() => answer("discard")} onCancel={() => answer("cancel")} /> : null;
}

// asked before a tab with unsaved changes is closed
function UnsavedChangesModal({ onSave, onDiscard, onCancel }: UnsavedChangesModalProps) {
    // takes the keys while it's up: cancel, or confirm which saves unless a button is focused (that one's pressed)
    useModal(
        "dialog",
        {
            confirm: () => (document.activeElement instanceof HTMLButtonElement ? document.activeElement.click() : onSave()),
            cancel: onCancel,
        },
        true,
        { passthrough: true }
    );

    const button = "px-3 h-6 border border-black text-sm";

    return (
        <div className="fixed inset-0 bg-black/20 flex items-center justify-center z-[9999] font-[Arial,sans-serif] select-none" onClick={onCancel}>
            <div className="bg-white border border-black w-80" onClick={(e) => e.stopPropagation()}>
                <div className="panel-header h-7 border-b border-black flex items-center pl-2 pr-2">Unsaved changes</div>
                <p className="px-2 py-3 text-sm">Save changes to this project before closing?</p>
                <div className="flex gap-2 justify-end px-2 pb-2">
                    <button className={`${button} mr-auto hover:bg-zinc-100`} onClick={onDiscard}>
                        Don't Save
                    </button>
                    <button className={`${button} hover:bg-zinc-100`} onClick={onCancel}>
                        Cancel
                    </button>
                    <button className={`${button} bg-black text-white hover:bg-zinc-800`} onClick={onSave}>
                        Save
                    </button>
                </div>
            </div>
        </div>
    );
}

export default UnsavedChangesModal;
