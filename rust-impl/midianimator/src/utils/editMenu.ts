import { invoke } from "@tauri-apps/api/core";
import { isTextField, registerKeymap } from "./keymap";

export { isTextField };

// undo/redo from the edit menu (or its shortcut) come to the focused window as app commands (src/utils/keymap.ts).
// cut/copy/paste/select all are the native menu items, they arrive as the usual copy/cut/paste events

// a focused text field undoes its own typing, anywhere else it's the graph's undo history (src-tauri/src/state/history.rs)
const undoRedo = (command: "undo" | "redo") => () => {
    if (isTextField(document.activeElement)) document.execCommand(command);
    else invoke(command === "undo" ? "history_undo" : "history_redo");
};
registerKeymap("app", { undo: undoRedo("undo"), redo: undoRedo("redo") });
