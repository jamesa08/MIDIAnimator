import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ReactFlowProvider } from "@xyflow/react";

import { useStateContext } from "../contexts/StateContext";
import { GroupContext, ScopedState } from "../contexts/GroupContext";
import { PROJECT_LOADED_EVENT } from "../utils/node";
import { GroupDef, Level, PATH_SEP, Project, allGroups, loadBuiltinGroups, resolvePath, withGroup, withoutGroup } from "../utils/groups";
import NodeGraphEditor, { ProjectAccess } from "./nodegraph/NodeGraphEditor";
import NodeGraphCanvas from "./nodegraph/NodeGraphCanvas";

const noop = () => {};

// the graph behind an open group, frozen and faded out, with the group node that's open outlined.
// it stays where it was looked at when the group was opened
function FrozenGraph({ level, openNodeId }: { level: Level; openNodeId: string }) {
    const nodes = useMemo(() => (level.graph.nodes ?? []).map(({ selected, dragging, ...n }: any) => ({ ...n, className: n.id === openNodeId ? "open-group-node" : undefined })), [level.graph.nodes, openNodeId]);
    const viewport = level.graph.viewport;
    return <NodeGraphCanvas frozen nodes={nodes} edges={level.graph.edges ?? []} defaultViewport={viewport} fitView={!viewport} fitViewOptions={{ maxZoom: 1 }} />;
}

type GraphViewProps = {
    levels: Level[];
    groups: Record<string, GroupDef>;
    project: ProjectAccess;
    specs: Record<string, any>;
    // false while it's getting ready behind the view on screen: drawn but see-through, and it takes no input
    shown: boolean;
    setPath: (path: string[]) => void;
    selections: Map<string, Set<string>>;
    onReady: (key: string) => void;
};

// one open graph: its editor, and while it's a group the graph it's in behind it, frozen and faded out
function GraphView({ levels, groups, project, specs, shown, setPath, selections, onReady }: GraphViewProps) {
    const openPath = levels.slice(1).map((l) => l.nodeId!);
    const key = openPath.join(PATH_SEP);
    const active = levels[levels.length - 1];
    const parent = levels.length > 1 ? levels[levels.length - 2] : null;
    const editable = active.groupId === null || !!project.get().groups?.[active.groupId];

    const openGroup = useCallback(
        (nodeId: string) => {
            const node = active.graph.nodes?.find((n: any) => n.id === nodeId);
            if (!node?.data?.group_id || !groups[node.data.group_id]) return;
            // a group can't be opened inside itself
            if (levels.some((l) => l.groupId === node.data.group_id)) return;
            setPath([...openPath, nodeId]);
        },
        [active, groups, levels, key, setPath]
    );

    const exitGroup = useCallback(
        (toRoot: boolean) => {
            if (openPath.length === 0) return;
            setPath(toRoot ? [] : openPath.slice(0, -1));
        },
        [key, setPath]
    );

    const ready = useCallback(() => onReady(key), [key, onReady]);
    const onLeave = useCallback((selected: string[]) => selections.set(key, new Set(selected)), [key, selections]);
    const initialSelection = useMemo(() => selections.get(key) ?? new Set<string>(), [key, selections]);
    // opacity, not visibility or display: React Flow sets visibility on every node, and needs layout to measure them
    const style = shown ? undefined : { opacity: 0, pointerEvents: "none" as const };

    const pathGroups = useMemo(() => levels.slice(1).map((l) => l.groupId!), [levels]);
    const groupContext = useMemo(() => ({ groups, scope: active.def, scopeId: active.groupId, editable, openGroup }), [groups, active, editable, openGroup]);
    const parentContext = useMemo(() => parent && { groups, scope: parent.def, scopeId: parent.groupId, editable: false, openGroup: noop }, [groups, parent]);

    return (
        <>
            {parent && parentContext && (
                <div className="graph-layer parent-layer" style={style}>
                    <GroupContext.Provider value={parentContext}>
                        <ScopedState path={openPath.slice(0, -1).join(PATH_SEP)}>
                            <ReactFlowProvider>
                                <FrozenGraph level={parent} openNodeId={active.nodeId!} />
                            </ReactFlowProvider>
                        </ScopedState>
                    </GroupContext.Provider>
                    <div className="parent-dim" />
                </div>
            )}
            <div className="graph-layer" style={style}>
                <GroupContext.Provider value={groupContext}>
                    <ScopedState path={key}>
                        <ReactFlowProvider>
                            <NodeGraphEditor level={active} path={openPath} pathGroups={pathGroups} editable={editable} project={project} specs={specs} openGroup={openGroup} exitGroup={exitGroup} initialSelection={initialSelection} onLeave={onLeave} onReady={ready} shown={shown} />
                        </ReactFlowProvider>
                    </ScopedState>
                </GroupContext.Provider>
            </div>
        </>
    );
}

// the node graph: the open graph's editor, and while a group is open the graph it's in behind it with a path back out.
// opening or leaving a group draws the next graph behind the scenes, the one on screen stays until it's ready.
// owns the project graph (the top-level graph plus the project's node groups) and sends it to the backend
function NodeGraph() {
    const { backEndState: state, setBackEndState: setState } = useStateContext();
    const [builtin, setBuiltin] = useState<Record<string, GroupDef>>({});
    // group node ids from the top level down to the graph asked for
    const [path, setPath] = useState<string[]>([]);
    // the graph on screen (a path), it changes to the one asked for once that one has finished drawing
    const [shownKey, setShownKey] = useState("");
    // what was selected in each graph when it was left, by path
    const selections = useRef(new Map<string, Set<string>>());

    useEffect(() => {
        loadBuiltinGroups().then(setBuiltin);
    }, []);

    // the latest project and state for callbacks, a backend push replaces the project
    const projectRef = useRef<Project>({ nodes: [], edges: [] });
    const stateRef = useRef(state);
    stateRef.current = state;
    const builtinRef = useRef(builtin);
    builtinRef.current = builtin;
    useEffect(() => {
        if (state.rf_instance) projectRef.current = state.rf_instance;
    }, [state.rf_instance]);

    const project: ProjectAccess = useMemo(
        () => ({
            get: () => projectRef.current,
            groups: (p?: Project) => allGroups(p ?? projectRef.current, builtinRef.current),
            commit: (p: Project, execute: boolean) => {
                projectRef.current = p;
                setState((s: any) => ({ ...s, rf_instance: p }));
                // only send the graph, the rest of our copy of the state may be stale
                invoke("js_update_graph", { rfInstance: JSON.stringify(p) });
                // Block execution if paused, a plain sync (moving nodes) doesn't need a re-run
                if (execute && !stateRef.current.execution_paused) {
                    invoke("execute_graph", { realtime: true });
                }
            },
        }),
        [setState]
    );

    // a new project starts with an empty graph, marked clean since there's nothing to save
    const initDone = useRef(false);
    useEffect(() => {
        if (!initDone.current && state.ready && state.rf_instance != undefined) {
            initDone.current = true;
            const empty = { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };
            projectRef.current = empty;
            setState((s: any) => ({ ...s, rf_instance: empty }));
            invoke("js_update_graph", { rfInstance: JSON.stringify(empty), clean: true });
        }
    }, [state.ready, state.rf_instance, setState]);

    // a loaded project starts at the top level
    useEffect(() => {
        const unlisten = listen(PROJECT_LOADED_EVENT, (event: any) => {
            projectRef.current = event.payload ?? { nodes: [], edges: [] };
            setPath([]);
        });
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    // the graphs down the path asked for, a group node that's gone (deleted, or over MCP) closes the groups from there
    const current: Project = state.rf_instance ?? { nodes: [], edges: [] };
    const groups = useMemo(() => allGroups(current, builtin), [current, builtin]);
    const levels = useMemo(() => resolvePath(current, groups, path), [current, groups, path]);
    const openPath = levels.slice(1).map((l) => l.nodeId!);
    const pathKey = openPath.join(PATH_SEP);
    useEffect(() => {
        if (openPath.length !== path.length) setPath(openPath);
    }, [openPath.length, path.length]);

    // the backend records the insides of the open groups, so they have values to show
    useEffect(() => {
        if (stateRef.current.ready) invoke("set_open_group", { path: pathKey });
    }, [pathKey, state.ready]);

    // the graph on screen
    const shownLevels = useMemo(() => resolvePath(current, groups, shownKey ? shownKey.split(PATH_SEP) : []), [current, groups, shownKey]);
    const shownPath = shownLevels.slice(1).map((l) => l.nodeId!);
    const shownPathKey = shownPath.join(PATH_SEP);
    const active = shownLevels[shownLevels.length - 1];
    const isBuiltin = active.groupId !== null && active.groupId in builtin;
    const isLocal = active.groupId !== null && !!current.groups?.[active.groupId];

    // the graph asked for takes over once it has finished drawing (only the latest one asked for)
    const pathKeyRef = useRef(pathKey);
    pathKeyRef.current = pathKey;
    const onReady = useCallback((key: string) => {
        if (key === pathKeyRef.current) setShownKey(key);
    }, []);

    // node specs by type, for the types and names of new group sockets
    const specs = useMemo(() => Object.fromEntries((state.default_nodes?.nodes ?? []).map((node: any) => [node.id, node])), [state.default_nodes]);

    // a built-in group is shared by reference: making it local copies it into the project, for every group node using it
    const makeLocal = useCallback(() => {
        if (active.groupId === null) return;
        project.commit(withGroup(project.get(), active.groupId, structuredClone(builtin[active.groupId])), true);
    }, [active.groupId, builtin, project]);

    const revertToBuiltin = useCallback(async () => {
        if (active.groupId === null) return;
        const name = groups[active.groupId]?.name ?? active.groupId;
        if (!(await ask(`Go back to the built-in "${name}"? Every change made to this project's copy is lost.`, { title: "Revert to Built-in", kind: "warning" }))) return;
        project.commit(withoutGroup(project.get(), active.groupId), true);
    }, [active.groupId, groups, project]);

    // the graph on screen, and the one asked for drawing on top of it (see-through) until it's ready
    const views = shownPathKey === pathKey ? [shownLevels] : [shownLevels, levels];

    return (
        <div className="node-graph-stack">
            {views.map((viewLevels) => {
                const key = viewLevels
                    .slice(1)
                    .map((l) => l.nodeId!)
                    .join(PATH_SEP);
                return <GraphView key={key} levels={viewLevels} groups={groups} project={project} specs={specs} shown={key === shownPathKey} setPath={setPath} selections={selections.current} onReady={onReady} />;
            })}
            {shownPath.length > 0 && (
                <div className="graph-overlay absolute top-2 z-10 flex items-center h-6 font-[Arial,sans-serif] text-xs select-none">
                    <button className="px-1 italic hover:underline" onClick={() => setPath([])}>
                        Root
                    </button>
                    {shownLevels.slice(1).map((level, i) => (
                        <span key={level.nodeId} className="flex items-center">
                            <span className="text-zinc-400">/</span>
                            <button className={`px-1 hover:underline ${i === shownLevels.length - 2 ? "font-bold" : ""}`} onClick={() => setPath(shownPath.slice(0, i + 1))}>
                                {level.def?.name ?? level.groupId}
                            </button>
                        </span>
                    ))}
                </div>
            )}
            {isBuiltin && (
                <div className="graph-overlay absolute top-9 z-10 flex items-center h-6 font-[Arial,sans-serif] text-xs select-none">
                    {isLocal ? (
                        <button className="px-3 h-6 border border-black bg-white hover:bg-zinc-100" onClick={revertToBuiltin}>
                            Revert to Built-in
                        </button>
                    ) : (
                        <button className="px-3 h-6 border border-black bg-black text-white hover:bg-zinc-800" onClick={makeLocal}>
                            Make Local
                        </button>
                    )}
                </div>
            )}
        </div>
    );
}

export default NodeGraph;
