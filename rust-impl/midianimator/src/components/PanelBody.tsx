import React, { useContext, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import nodeTypes from "../nodes/NodeTypes";
import { ReactFlowProvider } from "@xyflow/react";
import { PANELS, startDragGhost } from "../utils/panels";
import { StateContext } from "../contexts/StateContext";
import { GroupContext } from "../contexts/GroupContext";
import { GroupDef, allGroups, loadBuiltinGroups } from "../utils/groups";
import { NodeEntry, nodeEntries, previewData, useNodeSpecs } from "../utils/nodeEntries";
import HistoryList from "./HistoryList";
import PropertiesPanel, { SectionHeader } from "./PropertiesPanel";
import { CATEGORY_LABELS, nodeColors } from "../styles";
import { pushModal } from "../utils/keymap";

// where a node from the nodes panel was released, offset is where it was grabbed in node (unscaled) pixels
export interface PanelNodeDrop {
    nodeType: string;
    clientX: number;
    clientY: number;
    screenX: number;
    screenY: number;
    offsetX: number;
    offsetY: number;
}

// how the nodes panel shows its nodes: previews in a masonry grid, or a list of names
export type NodesView = "grid" | "list";

// the nodes panel's view, the panels.nodes_view setting so every window (docked and floating) shows the same one. kept
// here too so the header's buttons and the panel in this window change together, without waiting on the backend
let nodesView: NodesView = "grid";
const nodesViewListeners = new Set<() => void>();
const setNodesViewHere = (view: NodesView) => {
    nodesView = view;
    nodesViewListeners.forEach((listener) => listener());
};
let nodesViewLoaded = false;
const subscribeNodesView = (listener: () => void) => {
    if (!nodesViewLoaded) {
        nodesViewLoaded = true;
        const apply = (settings: any) => setNodesViewHere(settings?.panels?.nodes_view === "list" ? "list" : "grid");
        invoke("get_settings").then(apply);
        listen("settings_changed", (event: any) => apply(event.payload));
    }
    nodesViewListeners.add(listener);
    return () => {
        nodesViewListeners.delete(listener);
    };
};

export function useNodesView(): [NodesView, (view: NodesView) => void] {
    const view = useSyncExternalStore(subscribeNodesView, () => nodesView);
    const set = (next: NodesView) => {
        setNodesViewHere(next);
        invoke("set_setting", { path: "panels.nodes_view", value: next }).catch((e) => console.error(`Error saving setting panels.nodes_view: ${e}`));
    };
    return [view, set];
}

// the nodes panel's grid and list buttons
const NodesViewButtons: React.FC = () => {
    const [view, setView] = useNodesView();
    return (
        <div className="flex items-center gap-1 mr-2">
            <button className={`nodes-view-button${view === "grid" ? " active" : ""}`} aria-label="Grid" aria-pressed={view === "grid"} onClick={() => setView("grid")}>
                <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
                    <rect x="0" y="0" width="5" height="5" rx="1" />
                    <rect x="7" y="0" width="5" height="5" rx="1" />
                    <rect x="0" y="7" width="5" height="5" rx="1" />
                    <rect x="7" y="7" width="5" height="5" rx="1" />
                </svg>
            </button>
            <button className={`nodes-view-button${view === "list" ? " active" : ""}`} aria-label="List" aria-pressed={view === "list"} onClick={() => setView("list")}>
                <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
                    <rect x="0" y="1" width="2" height="2" />
                    <rect x="4" y="1.4" width="8" height="1.2" />
                    <rect x="0" y="5" width="2" height="2" />
                    <rect x="4" y="5.4" width="8" height="1.2" />
                    <rect x="0" y="9" width="2" height="2" />
                    <rect x="4" y="9.4" width="8" height="1.2" />
                </svg>
            </button>
        </div>
    );
};

// buttons a panel has in its header (docked and floating), before popping out or docking
export const PanelHeaderButtons: React.FC<{ id: number }> = ({ id }) => (PANELS[id]?.name === "Nodes" ? <NodesViewButtons /> : null);

interface PanelBodyProps {
    id: number;
    onNodeDrop: (drop: PanelNodeDrop) => void;
    // draw the dragged node in the drag ghost window so it can follow the cursor out of this window
    ghostWindow?: boolean;
}

// preview of one node, zoomed out at its own width (index.css .node.preview). an inline block so no engine splits it
// across columns (webkit does with break-inside alone), the frame inside is the node's size so it can be centered.
// kept outside PanelBody, a component declared inside another is a new type every render and remounts
const ScaledNodeWrapper: React.FC<{ label: string; Node: any; data: any; onPointerDown: (event: React.PointerEvent<HTMLDivElement>) => void }> = ({ label, Node, data, onPointerDown }) => (
    <div className="node-container inline-block w-full align-top mb-2 break-inside-avoid">
        <div className="node-frame w-fit max-w-full" data-label={label} onPointerDown={onPointerDown}>
            <Node data={data} />
        </div>
    </div>
);

// a node's row in the list view, in its header's color (a built-in group's is its category's, nodes/group.tsx). the node
// is drawn hidden under it for the drag ghost
const NodeRow: React.FC<{ entry: NodeEntry; color: string; Node: any; data: any; onPointerDown: (event: React.PointerEvent<HTMLDivElement>) => void }> = ({ entry, color, Node, data, onPointerDown }) => (
    <div className="nodes-list-row" style={nodeColors(color)} onPointerDown={onPointerDown}>
        <span />
        <div className="nodes-list-name">
            <span className="nodes-list-swatch" />
            <span className="truncate">{entry.label}</span>
        </div>
        <div className="absolute inset-x-0 top-0 h-0 overflow-hidden">
            <div className="node-frame w-fit max-w-full">
                <Node data={data} />
            </div>
        </div>
    </div>
);

// the built-in groups plus the project's own (when this window has the project state), for group node previews
export function usePanelGroups(): Record<string, GroupDef> {
    const [builtin, setBuiltin] = useState<Record<string, GroupDef>>({});
    useEffect(() => {
        loadBuiltinGroups().then(setBuiltin);
    }, []);
    const project = useContext(StateContext)?.backEndState?.rf_instance;
    return useMemo(() => allGroups(project, builtin), [project, builtin]);
}

// panel contents, shared by the docked panel and its popped out window
const PanelBody: React.FC<PanelBodyProps> = ({ id, onNodeDrop, ghostWindow = false }) => {
    // drag a preview node out of the panel, the node graph adds it where it's released.
    // pointer events instead of html5 drag and drop, tauri's native drop handling swallows html5 drops
    const startNodeDrag = (event: React.PointerEvent<HTMLDivElement>, nodeType: string) => {
        if (event.button !== 0) return;
        const frame = event.currentTarget.classList.contains("node-frame") ? event.currentTarget : event.currentTarget.querySelector(".node-frame");
        const preview = frame?.querySelector(".node.preview") as HTMLElement | null;
        if (!frame || !preview) return;

        // previews are zoomed out, the grab offset is measured on their frame (the node's size but not zoomed, engines
        // measure zoomed elements differently) and kept in real node pixels. a list row's node is hidden, it's held near
        // its header's top left
        const rect = frame.getBoundingClientRect();
        const scale = parseFloat(getComputedStyle(preview).zoom) || 1;
        const grabX = frame === event.currentTarget ? event.clientX - rect.left : 12;
        const grabY = frame === event.currentTarget ? event.clientY - rect.top : 8;
        const startX = event.clientX;
        const startY = event.clientY;
        let ghost: HTMLElement | null = null;
        let dragging = false;
        // ghost window version, set up now so the node is ready by the time the drag starts
        const ghostWin = ghostWindow ? startDragGhost(nodeType, rect.width) : null;
        // the drag takes the keys, cancelling (escape) drops nothing
        const endModal = pushModal("drag", { cancel: () => cleanup() });

        const handleMove = (e: PointerEvent) => {
            // small dead zone so a plain click doesn't start a drag
            if (!dragging && Math.hypot(e.clientX - startX, e.clientY - startY) < 4) return;
            if (!dragging) {
                dragging = true;
                document.body.style.cursor = "grabbing";
                if (!ghostWin) {
                    // clone of the preview that follows the cursor, in a box the frame's width so it's sized like in the panel
                    ghost = document.createElement("div");
                    Object.assign(ghost.style, { position: "fixed", left: "0", top: "0", width: `${rect.width}px`, opacity: "0.75", pointerEvents: "none", zIndex: "2000", cursor: "grabbing" });
                    ghost.appendChild(preview.cloneNode(true));
                    document.body.appendChild(ghost);
                }
            }
            if (ghostWin) ghostWin.move(e.screenX - grabX, e.screenY - grabY);
            if (ghost) ghost.style.transform = `translate(${e.clientX - grabX}px, ${e.clientY - grabY}px)`;
        };

        const cleanup = () => {
            endModal();
            ghostWin?.end();
            ghost?.remove();
            document.body.style.cursor = "";
            window.removeEventListener("pointermove", handleMove);
            window.removeEventListener("pointerup", handleUp);
        };

        const handleUp = (e: PointerEvent) => {
            cleanup();
            if (!dragging) return;
            onNodeDrop({ nodeType, clientX: e.clientX, clientY: e.clientY, screenX: e.screenX, screenY: e.screenY, offsetX: grabX / scale, offsetY: grabY / scale });
        };

        event.preventDefault();
        window.addEventListener("pointermove", handleMove);
        window.addEventListener("pointerup", handleUp);
    };

    const groups = usePanelGroups();
    const specs = useNodeSpecs();
    const entries = useMemo(() => nodeEntries(groups, specs), [groups, specs]);
    const groupContext = useMemo(() => ({ groups, scope: null, scopeId: null, editable: false, openGroup: () => {} }), [groups]);
    const [view] = useNodesView();
    // the entries split by category, in the order nodeEntries sorted them
    const sections = useMemo(() => {
        const byCategory = new Map<string, NodeEntry[]>();
        for (const entry of entries) byCategory.set(entry.category, [...(byCategory.get(entry.category) ?? []), entry]);
        return [...byCategory];
    }, [entries]);
    const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
    const toggle = (category: string) =>
        setCollapsed((prev) => {
            const next = new Set(prev);
            if (!next.delete(category)) next.add(category);
            return next;
        });

    if (PANELS[id]?.name === "History") return <HistoryList />;
    if (PANELS[id]?.name === "Properties") return <PropertiesPanel />;
    if (PANELS[id]?.name !== "Nodes") return null;

    return (
        <ReactFlowProvider>
            <GroupContext.Provider value={groupContext}>
                {/* the list view is drawn like the properties panel's grid */}
                <div className={`nodes-panel${view === "list" ? " properties" : ""}`}>
                    {sections.map(([category, sectionEntries]) => (
                        <React.Fragment key={category}>
                            <SectionHeader title={CATEGORY_LABELS[category] ?? category} collapsed={collapsed.has(category)} onToggle={() => toggle(category)} />
                            {/* masonry: as many columns as fit at the widest node's scaled down width (index.css .node.preview), at most 3 */}
                            {!collapsed.has(category) && view === "grid" && (
                                <div className="nodes-grid columns-[140px_3] gap-2 p-2 pb-0">
                                    {sectionEntries.map((entry) => (
                                        <ScaledNodeWrapper key={entry.key} label={entry.label} Node={(nodeTypes as any)[entry.nodeType]} data={previewData(entry)} onPointerDown={(e) => startNodeDrag(e, entry.key)} />
                                    ))}
                                </div>
                            )}
                            {!collapsed.has(category) && view === "list" && (
                                <div className="nodes-list">
                                    {sectionEntries.map((entry) => (
                                        <NodeRow key={entry.key} entry={entry} color={groups[entry.data.group_id]?.category ? `${groups[entry.data.group_id].category}_group` : entry.category} Node={(nodeTypes as any)[entry.nodeType]} data={previewData(entry)} onPointerDown={(e) => startNodeDrag(e, entry.key)} />
                                    ))}
                                </div>
                            )}
                        </React.Fragment>
                    ))}
                </div>
            </GroupContext.Provider>
        </ReactFlowProvider>
    );
};

export default PanelBody;
