import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createContext, useCallback, useContext, useEffect, useMemo } from "react";
import { useStateContext } from "../contexts/StateContext";
import { useGroupContext } from "../contexts/GroupContext";

// every change to the project graph is an op applied by the backend (Op in src-tauri/src/graph/ops.rs), which records it
// for undo and sends the graph back. connections are in data-flow terms: from an output to an input
type XY = { x: number; y: number };
// one socket of a node (src-tauri/src/graph/sockets.rs)
export type SocketRef = { node: string; side: "inputs" | "outputs"; socket: string };
export type Op =
    | { op: "add_nodes"; nodes: { type: string; data?: any; position: XY }[] }
    | { op: "delete"; nodes?: string[]; edges?: string[]; sockets?: SocketRef[] }
    | { op: "connect"; from_node: string; from_output: string; to_node: string; to_input: string }
    | { op: "set_inputs"; node: string; inputs: Record<string, any> }
    | { op: "set_tag"; node: string; side: "inputs" | "outputs"; socket: string; name: string }
    | { op: "set_tags"; sockets: SocketRef[]; name: string }
    | { op: "set_label"; node: string; label: string }
    | { op: "move"; positions: Record<string, XY> }
    | { op: "resize"; node: string; width: number; height: number; position: XY }
    | { op: "select"; nodes: string[]; edges: string[]; sockets: SocketRef[] }
    | { op: "duplicate"; nodes: string[]; offset: XY }
    | { op: "paste"; text: string; position: XY }
    | { op: "cut"; nodes?: string[]; edges?: string[]; sockets?: SocketRef[] }
    | { op: "group"; nodes: string[]; widths: Record<string, number> }
    | { op: "ungroup"; nodes: string[] }
    | { op: "rename_socket"; side: "inputs" | "outputs"; id: string; name: string }
    | { op: "remove_socket"; side: "inputs" | "outputs"; id: string }
    | { op: "make_local" }
    | { op: "revert_group" }
    | { op: "viewport"; viewport: any };

// a tab's graph after an edit and the nodes it added
export type Applied = { tab: string; graph_rev: number; rf_instance: any; added: { id: string; position: XY }[] };

// the tab the graph below it belongs to, its edits go to that tab even if another one is shown by the time they land
export const TabContext = createContext<string>("");

// `txn`: ops with the same transaction are one undo step until it's ended or cancelled (a grab after adding or duplicating).
// `commitToHistory`: false for no undo step (values filled in automatically), true when left out
export type ApplyOptions = { txn?: string; commitToHistory?: boolean };

// the state with a tab's graph from the backend: only the tab on screen's, and not if the state already has a newer one
// (graph_rev only goes up)
export function takeGraph(state: any, graph: { tab: string; graph_rev?: number; rf_instance: any }): any {
    if (graph.tab !== state.active_tab || (graph.graph_rev ?? 0) < (state.graph_rev ?? 0)) return state;
    return { ...state, graph_rev: graph.graph_rev, rf_instance: graph.rf_instance };
}

// a whole state from the backend unless the state is newer (state_rev only goes up), keeping the graph the state has if
// it's the same tab's and newer
export function takeState(state: any, next: any): any {
    if ((next.state_rev ?? 0) < (state.state_rev ?? 0)) return state;
    if (next.active_tab === state.active_tab && (next.graph_rev ?? 0) < (state.graph_rev ?? 0)) return { ...next, graph_rev: state.graph_rev, rf_instance: state.rf_instance };
    return next;
}

// keeps the state of a window other than the main one (a floating panel) up to date, App does it for the main window
export function useStateSync() {
    const { setBackEndState } = useStateContext();
    useEffect(() => {
        invoke("get_state").then((state) => setBackEndState((s: any) => takeState(s, state)));
        const stateListener = listen("update_state", (event: any) => setBackEndState((s: any) => takeState(s, event.payload)));
        const graphListener = listen("graph_changed", (event: any) => setBackEndState((s: any) => takeGraph(s, event.payload)));
        return () => {
            stateListener.then((f) => f());
            graphListener.then((f) => f());
        };
    }, []);
}

// applies ops to the graph `scope` (a group id, null for the top level) of the tab, see graph_apply in src-tauri/src/state/graph.rs.
// copy, cut and paste use the system clipboard from the backend. the graph that comes back is taken right away, so it's
// there when the promise resolves
export function useGraphOps(scope: string | null) {
    const { setBackEndState } = useStateContext();
    const tab = useContext(TabContext);
    return useMemo(() => {
        const edit = async (command: string, args: Record<string, any>): Promise<Applied> => {
            const applied = await invoke<Applied>(command, { tab, scope, ...args });
            setBackEndState((s: any) => takeGraph(s, applied));
            return applied;
        };
        return {
            apply: (ops: Op[], options: ApplyOptions = {}) => edit("graph_apply", { ops, txn: options.txn ?? null, commitToHistory: options.commitToHistory ?? true }),
            copy: (nodes: string[]) => invoke("graph_copy", { tab, scope, nodes }),
            cut: (nodes: string[], edges: string[], sockets: SocketRef[]) => edit("graph_cut", { nodes, edges, sockets }),
            paste: (position: XY) => edit("graph_paste", { position }),
            end: (txn: string) => invoke("history_end", { tab, txn }),
            cancel: (txn: string) => invoke("history_cancel", { tab, txn }),
        };
    }, [tab, scope, setBackEndState]);
}

// sets values on a node's inputs (`null` unsets one), for node components. nothing happens in a graph that can't be edited
export function useSetInputs() {
    const { scopeId, editable } = useGroupContext();
    const { apply } = useGraphOps(scopeId);
    return useCallback(
        (node: string, inputs: Record<string, any>, options: ApplyOptions = {}) => {
            if (!editable) return;
            apply([{ op: "set_inputs", node, inputs }], options).catch((e) => console.error(`set_inputs on ${node}: ${e}`));
        },
        [apply, editable]
    );
}
