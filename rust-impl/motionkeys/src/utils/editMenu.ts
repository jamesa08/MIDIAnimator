import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTextField, registerKeymap } from "./keymap";
import { historyLabel } from "./history";
import { queued } from "./graphOps";
import { showStatus } from "./status";

export { isTextField };

// undo/redo from the edit menu (or its shortcut) come to the focused window as app commands (src/utils/keymap.ts).
// cut/copy/paste/select all are the native menu items, they arrive as the usual copy/cut/paste events

// the tab on screen as this window last heard it (state_rev only goes up). undo and redo name it and wait for its ops
// before them, the backend refuses them once another tab is shown. until a state comes, the tab the history is of
let shown = { tab: "", rev: -1 };
listen("update_state", (event: any) => {
    const state = event.payload;
    if (typeof state?.active_tab === "string" && (state.state_rev ?? 0) >= shown.rev) shown = { tab: state.active_tab, rev: state.state_rev ?? 0 };
}).then(() =>
    invoke<{ tab: string }>("get_history").then(
        (history) => {
            if (shown.rev < 0 && typeof history?.tab === "string") shown = { tab: history.tab, rev: -1 };
        },
        () => {}
    )
);

// a focused text field undoes its own typing, anywhere else it's the graph's undo history (src-tauri/src/state/history.rs)
const undoRedo = (command: "undo" | "redo") => () => {
    if (isTextField(document.activeElement)) {
        document.execCommand(command);
        return;
    }
    const { tab } = shown;
    queued(tab, () => invoke<{ op: string } | null>("history_step", { tab: tab || null, redo: command === "redo" }))
        .then((step) => {
            if (step) showStatus(`${command === "undo" ? "Undo" : "Redo"} ${historyLabel(step.op)}`);
        })
        .catch((e) => console.error(`${command}: ${e}`));
};
registerKeymap("app", { undo: undoRedo("undo"), redo: undoRedo("redo") });
