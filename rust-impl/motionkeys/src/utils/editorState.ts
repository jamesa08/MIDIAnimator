import { useSyncExternalStore } from "react";
import { NodeEntry } from "./nodeEntries";
import { pushMode } from "./keymap";

// what the node editor on screen has, for the toolbar (src/components/ToolBar.tsx) to know which of its buttons can run.
// the editor shown says so (NodeGraphEditor), the note map while it covers the editor (NoteMapView)

// `selected` nodes, `groups` the selected group nodes, `openable` the selected groups and note maps. `entries` are the nodes
// that can be added here, none in the note map
export type EditorState = { editable: boolean; selected: number; groups: number; openable: number; inGroup: boolean; entries: NodeEntry[] | null };

let state: EditorState = { editable: false, selected: 0, groups: 0, openable: 0, inGroup: false, entries: null };
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
};

export function setEditorState(next: EditorState) {
    if ((Object.keys(next) as (keyof EditorState)[]).every((key) => next[key] === state[key])) return;
    state = next;
    listeners.forEach((listener) => listener());
}

export function useEditorState(): EditorState {
    return useSyncExternalStore(subscribe, () => state);
}

// the canvas's tool, this window's own and never saved. "select" is the graph's usual clicks and drags, the others take
// the left drag anywhere, over nodes too (NodeGraphCanvas): "box_select" draws a box, "pan" moves the view, "zoom" zooms it
export type CanvasTool = "select" | "box_select" | "pan" | "zoom";

let tool: CanvasTool = "select";
// the keys of the tool that's on (its mode in the keymap), escape goes back to select
let popToolKeys: (() => void) | null = null;

export function setCanvasTool(next: CanvasTool) {
    if (next === tool) return;
    tool = next;
    popToolKeys?.();
    popToolKeys = next === "select" ? null : pushMode("tool", { cancel: () => setCanvasTool("select") });
    listeners.forEach((listener) => listener());
}

// the keymap's commands that pick a tool, for the editors that show a canvas
export const canvasToolCommands = {
    select_tool: () => setCanvasTool("select"),
    box_select_tool: () => setCanvasTool("box_select"),
    pan_tool: () => setCanvasTool("pan"),
    zoom_tool: () => setCanvasTool("zoom"),
};

export function useCanvasTool(): CanvasTool {
    return useSyncExternalStore(subscribe, () => tool);
}

// sent by the toolbar's add menu with the entry's key, the editor on screen adds it under the cursor and grabs it
export const TOOLBAR_ADD_EVENT = "toolbar-add-node";
