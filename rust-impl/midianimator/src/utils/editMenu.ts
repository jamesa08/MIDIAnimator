import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// undo/redo from the edit menu (or its shortcut) are sent to the focused window only, see EDIT_EVENT in src-tauri/src/ui/menu.rs.
// cut/copy/paste/select all are the native menu items, they arrive as the usual copy/cut/paste events
export const EDIT_EVENT = "menu-edit";

// true when the element takes typing, it keeps its own undo/redo and clipboard
export function isTextField(element: Element | null): boolean {
    const el = element as HTMLElement | null;
    return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
}

// a focused text field undoes its own typing, anywhere else it's the graph's undo history (src-tauri/src/state/history.rs)
getCurrentWebviewWindow().listen<"undo" | "redo">(EDIT_EVENT, (event) => {
    if (isTextField(document.activeElement)) document.execCommand(event.payload);
    else invoke(event.payload === "undo" ? "history_undo" : "history_redo");
});
