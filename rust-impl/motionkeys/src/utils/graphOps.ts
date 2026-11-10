import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef } from "react";
import { useStateContext } from "../contexts/StateContext";
import { useGroupContext } from "../contexts/GroupContext";

// every change to the project graph is an op applied by the backend (Op in src-tauri/src/graph/ops.rs), which records it
// for undo and sends the graph back. connections are in data-flow terms: from an output to an input
type XY = { x: number; y: number };
// one socket of a node (src-tauri/src/graph/sockets.rs)
export type SocketRef = { node: string; side: "inputs" | "outputs"; socket: string };
export type Op =
    | { op: "add_nodes"; nodes: { type: string; data?: any; position: XY; frame?: string }[] }
    | { op: "delete"; nodes?: string[]; edges?: string[]; sockets?: SocketRef[]; keep_frames?: boolean }
    | { op: "connect"; from_node: string; from_output: string; to_node: string; to_input: string }
    | { op: "set_inputs"; node: string; inputs: Record<string, any> }
    | { op: "set_tag"; node: string; side: "inputs" | "outputs"; socket: string; name: string }
    | { op: "set_tags"; sockets: SocketRef[]; name: string }
    | { op: "set_label"; node: string; label: string }
    | { op: "collapse"; nodes: string[]; collapsed: boolean }
    | { op: "linked_only"; node: string; linked_only: boolean }
    | { op: "map_view"; node: string; view: "" | "notes" | "animations" }
    | { op: "move"; positions: Record<string, XY>; frames?: Record<string, string | null> }
    | { op: "resize"; node: string; width: number; height: number | null; position: XY }
    | { op: "select"; nodes: string[]; edges: string[]; sockets: SocketRef[] }
    | { op: "duplicate"; nodes: string[]; offset: XY }
    | { op: "paste"; text: string; position: XY }
    | { op: "cut"; nodes?: string[]; edges?: string[]; sockets?: SocketRef[] }
    | { op: "group"; nodes: string[]; widths: Record<string, number> }
    | { op: "ungroup"; nodes: string[] }
    | { op: "join_in_frame"; nodes: string[] }
    | { op: "remove_from_frame"; nodes: string[] }
    | { op: "set_frame"; node: string; color?: string; shrink?: boolean; lock_size?: boolean; locked?: boolean; label_size?: number; label_align?: string }
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
// `commitToHistory`: false for no undo step (values filled in automatically), true when left out. `sizes`: the nodes'
// sizes on screen, frames fit around them
export type ApplyOptions = { txn?: string; commitToHistory?: boolean; sizes?: Record<string, { width: number; height: number }> };

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

// each tab's last op on its way (or its reply), the next one waits for it. the backend takes concurrent commands in any
// order, so a tab's edits, undos and transaction ends go one at a time in the order they were made
const queues = new Map<string, Promise<unknown>>();

// runs `send` (a command for `tab`) once the tab's ops before it have their replies, resolves to its reply
export function queued<T>(tab: string, send: () => Promise<T>): Promise<T> {
    const reply = (queues.get(tab) ?? Promise.resolve()).then(send);
    // a failed op doesn't hold up the ones after it
    const done = reply.catch(() => {});
    queues.set(tab, done);
    done.then(() => {
        if (queues.get(tab) === done) queues.delete(tab);
    });
    return reply;
}

// applies ops to the graph `scope` (a group id, null for the top level) of the tab, see graph_apply in src-tauri/src/state/graph.rs.
// copy, cut and paste use the system clipboard from the backend. the graph that comes back is taken right away, so it's
// there when the promise resolves. every command waits for the tab's ops before it (`queued`). `cancel` resolves to
// false when the transaction couldn't all be undone (another step ended it first), what it added is still there
export function useGraphOps(scope: string | null) {
    const { setBackEndState } = useStateContext();
    const tab = useContext(TabContext);
    return useMemo(() => {
        const edit = (command: string, args: Record<string, any>): Promise<Applied> =>
            queued(tab, async () => {
                const applied = await invoke<Applied>(command, { tab, scope, ...args });
                setBackEndState((s: any) => takeGraph(s, applied));
                return applied;
            });
        return {
            apply: (ops: Op[], options: ApplyOptions = {}) => edit("graph_apply", { ops, txn: options.txn ?? null, commitToHistory: options.commitToHistory ?? true, sizes: options.sizes ?? null }),
            copy: (nodes: string[]) => queued(tab, () => invoke("graph_copy", { tab, scope, nodes })),
            cut: (nodes: string[], edges: string[], sockets: SocketRef[]) => edit("graph_cut", { nodes, edges, sockets }),
            paste: (position: XY) => edit("graph_paste", { position }),
            unused: () => queued(tab, () => invoke<string[]>("graph_unused", { tab, scope })),
            end: (txn: string) => queued(tab, () => invoke("history_end", { tab, txn })),
            cancel: (txn: string) => queued(tab, () => invoke<boolean>("history_cancel", { tab, txn })),
        };
    }, [tab, scope, setBackEndState]);
}

// sets values on a node's inputs (`null` unsets one), for node components. nothing happens in a graph that can't be edited.
// the promise settles once the backend has it
export function useSetInputs() {
    const { scopeId, editable } = useGroupContext();
    const { apply } = useGraphOps(scopeId);
    return useCallback(
        (node: string, inputs: Record<string, any>, options: ApplyOptions = {}): Promise<unknown> => {
            if (!editable) return Promise.resolve();
            return apply([{ op: "set_inputs", node, inputs }], options).catch((e) => console.error(`set_inputs on ${node}: ${e}`));
        },
        [apply, editable]
    );
}

// a drag that sets a node's inputs (a slider, a line on a curve). the inputs stay with the drag until it ends, the node
// shows where it's dragged to: a send every frame would bring the whole graph back each time and redraw every node.
// when it ends they go out as one undo step and the graph runs once
type Drag = { pending: Record<string, any> | null };

// `start` and `stop` a drag, `send` inputs during it (or on their own, outside one, as their own undo step)
export function useInputDrag(id: string) {
    const setInputs = useSetInputs();
    const dragRef = useRef<Drag | null>(null);
    const send = (inputs: Record<string, any>) => {
        const drag = dragRef.current;
        if (!drag) {
            setInputs(id, inputs);
            return;
        }
        drag.pending = { ...(drag.pending ?? {}), ...inputs };
    };
    const start = () => (dragRef.current = { pending: null });
    const stop = () => {
        const drag = dragRef.current;
        dragRef.current = null;
        if (drag?.pending) setInputs(id, drag.pending);
    };
    return { start, send, stop, dragging: () => dragRef.current !== null };
}

// collapses a node to its header or expands it, for node components. nothing happens in a graph that can't be edited
export function useCollapse() {
    const { scopeId, editable } = useGroupContext();
    const { apply } = useGraphOps(scopeId);
    return useCallback(
        (node: string, collapsed: boolean) => {
            if (!editable) return;
            apply([{ op: "collapse", nodes: [node], collapsed }]).catch((e) => console.error(`collapse ${node}: ${e}`));
        },
        [apply, editable]
    );
}
