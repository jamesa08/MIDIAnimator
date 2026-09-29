import { createContext, useContext, useMemo } from "react";
import { StateContext, useStateContext } from "./StateContext";
import { GroupDef, scopedValues } from "../utils/groups";

// what nodes need to know about the graph they're in: the groups group nodes can run, the group being edited
// (group input and output get their sockets from it), and how to open a group node
type GroupContextValue = {
    groups: Record<string, GroupDef>;
    // the group whose graph this is, null for the top level
    scope: GroupDef | null;
    scopeId: string | null;
    // false inside a built-in group that hasn't been made local, and for the frozen parent graph
    editable: boolean;
    // opens a group node in this graph (Tab)
    openGroup: (nodeId: string) => void;
};

export const GroupContext = createContext<GroupContextValue>({ groups: {}, scope: null, scopeId: null, editable: true, openGroup: () => {} });

export const useGroupContext = () => useContext(GroupContext);

// shows the nodes inside the group node at `path` the values recorded for them, keyed by their own ids,
// so node components read `executed_results[id]` the same inside a group as at the top level
export function ScopedState({ path, children }: { path: string; children: React.ReactNode }) {
    const context = useStateContext();
    const state = context.backEndState;
    const scoped = useMemo(() => (path === "" ? state : { ...state, executed_results: scopedValues(state.executed_results, path), executed_inputs: scopedValues(state.executed_inputs, path) }), [state, path]);
    return <StateContext.Provider value={{ ...context, backEndState: scoped }}>{children}</StateContext.Provider>;
}
