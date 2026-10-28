import { useSyncExternalStore } from "react";
import { NodeEntry } from "./nodeEntries";

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

// sent by the toolbar's add menu with the entry's key, the editor on screen adds it under the cursor and grabs it
export const TOOLBAR_ADD_EVENT = "toolbar-add-node";
