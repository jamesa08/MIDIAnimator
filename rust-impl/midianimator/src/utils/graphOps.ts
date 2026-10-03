import { invoke } from "@tauri-apps/api/core";
import { useCallback, useMemo } from "react";
import { useStateContext } from "../contexts/StateContext";
import { useGroupContext } from "../contexts/GroupContext";

// every change to the project graph is an op applied by the backend (Op in src-tauri/src/graph/ops.rs), which records it
// for undo and sends the graph back. connections are in data-flow terms: from an output to an input
type XY = { x: number; y: number };
export type Op =
    | { op: "add_nodes"; nodes: { type: string; data?: any; position: XY }[] }
    | { op: "delete"; nodes?: string[]; edges?: string[] }
    | { op: "connect"; from_node: string; from_output: string; to_node: string; to_input: string }
    | { op: "set_inputs"; node: string; inputs: Record<string, any> }
    | { op: "move"; positions: Record<string, XY> }
    | { op: "resize"; node: string; width: number; height: number; position: XY }
    | { op: "select"; nodes: string[]; edges: string[] }
    | { op: "duplicate"; nodes: string[]; offset: XY }
    | { op: "group"; nodes: string[]; widths: Record<string, number> }
    | { op: "ungroup"; nodes: string[] }
    | { op: "rename_socket"; side: "inputs" | "outputs"; id: string; name: string }
    | { op: "remove_socket"; side: "inputs" | "outputs"; id: string }
    | { op: "make_local" }
    | { op: "revert_group" }
    | { op: "viewport"; viewport: any };

// the graph after an edit and the nodes it added
export type Applied = { graph_rev: number; rf_instance: any; added: { id: string; position: XY }[] };

// `txn`: ops with the same transaction are one undo step until it's ended or cancelled (a grab after adding or duplicating).
// `commitToHistory`: false for no undo step (values filled in automatically), true when left out
export type ApplyOptions = { txn?: string; commitToHistory?: boolean };

// the state with a graph from the backend, unless the state already has a newer one (graph_rev only goes up)
export function takeGraph(state: any, graph: { graph_rev?: number; rf_instance: any }): any {
    if ((graph.graph_rev ?? 0) < (state.graph_rev ?? 0)) return state;
    return { ...state, graph_rev: graph.graph_rev, rf_instance: graph.rf_instance };
}

// a whole state from the backend, keeping the graph the state has if it's newer
export function takeState(state: any, next: any): any {
    if ((next.graph_rev ?? 0) < (state.graph_rev ?? 0)) return { ...next, graph_rev: state.graph_rev, rf_instance: state.rf_instance };
    return next;
}

// applies ops to the graph `scope` (a group id, null for the top level), see graph_apply in src-tauri/src/state/graph.rs.
// the graph that comes back is taken right away, so it's there when the promise resolves
export function useGraphOps(scope: string | null) {
    const { setBackEndState } = useStateContext();
    return useMemo(
        () => ({
            apply: async (ops: Op[], options: ApplyOptions = {}): Promise<Applied> => {
                const applied = await invoke<Applied>("graph_apply", { scope, ops, txn: options.txn ?? null, commitToHistory: options.commitToHistory ?? true });
                setBackEndState((s: any) => takeGraph(s, applied));
                return applied;
            },
            end: (txn: string) => invoke("history_end", { txn }),
            cancel: (txn: string) => invoke("history_cancel", { txn }),
        }),
        [scope, setBackEndState]
    );
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
