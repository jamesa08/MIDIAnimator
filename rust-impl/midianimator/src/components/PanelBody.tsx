import React, { useContext, useEffect, useMemo, useRef, useState } from "react";
import nodeTypes from "../nodes/NodeTypes";
import { ReactFlowProvider } from "@xyflow/react";
import { PANELS, startDragGhost } from "../utils/panels";
import { StateContext } from "../contexts/StateContext";
import { GroupContext } from "../contexts/GroupContext";
import { GroupDef, allGroups, loadBuiltinGroups } from "../utils/groups";
import { nodeEntries, previewData, useNodeSpecs } from "../utils/nodeEntries";
import HistoryList from "./HistoryList";
import PropertiesPanel from "./PropertiesPanel";
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

interface PanelBodyProps {
    id: number;
    onNodeDrop: (drop: PanelNodeDrop) => void;
    // draw the dragged node in the drag ghost window so it can follow the cursor out of this window
    ghostWindow?: boolean;
}

// preview of one node, scaled down to half size.
// kept outside PanelBody, a component declared inside another is a new type every render and remounts
const ScaledNodeWrapper: React.FC<{ Node: any; data: any; onPointerDown: (event: React.PointerEvent<HTMLDivElement>) => void }> = ({ Node, data, onPointerDown }) => {
    const nodeRef = useRef<HTMLDivElement>(null);

    // the scale doesn't shrink the space the node takes up, a negative margin gives back the lower half.
    // kept in sync with the node's size, it can render already filled in, change size or remount at any time
    useEffect(() => {
        const node = nodeRef.current?.querySelector(".node.preview") as HTMLElement | null;
        if (!node) return;
        const observer = new ResizeObserver(() => {
            node.style.marginBottom = `-${node.offsetHeight * 0.5}px`;
        });
        observer.observe(node);
        return () => observer.disconnect();
    }, []);

    return (
        <div ref={nodeRef} className="node-container" onPointerDown={onPointerDown}>
            <Node data={data} />
        </div>
    );
};

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
        const preview = event.currentTarget.querySelector(".node.preview") as HTMLElement | null;
        if (!preview) return;

        // previews are drawn at half scale, grab offset is kept in real node pixels
        const rect = preview.getBoundingClientRect();
        const scale = rect.width / preview.offsetWidth || 0.5;
        const grabX = event.clientX - rect.left;
        const grabY = event.clientY - rect.top;
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
                    // clone of the preview that follows the cursor
                    ghost = preview.cloneNode(true) as HTMLElement;
                    Object.assign(ghost.style, { position: "fixed", left: "0", top: "0", width: `${preview.offsetWidth}px`, margin: "0", opacity: "0.75", pointerEvents: "none", zIndex: "2000", transformOrigin: "top left", cursor: "grabbing" });
                    document.body.appendChild(ghost);
                }
            }
            if (ghostWin) ghostWin.move(e.screenX - grabX, e.screenY - grabY);
            if (ghost) ghost.style.transform = `translate(${e.clientX - grabX}px, ${e.clientY - grabY}px) scale(${scale})`;
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

    if (PANELS[id]?.name === "History") return <HistoryList />;
    if (PANELS[id]?.name === "Properties") return <PropertiesPanel />;
    if (PANELS[id]?.name !== "Nodes") return null;

    return (
        <ReactFlowProvider>
            <GroupContext.Provider value={groupContext}>
                <div className="nodes-grid p-2">
                    {entries.map((entry) => (
                        <ScaledNodeWrapper key={entry.key} Node={(nodeTypes as any)[entry.nodeType]} data={previewData(entry)} onPointerDown={(e) => startNodeDrag(e, entry.key)} />
                    ))}
                </div>
            </GroupContext.Provider>
        </ReactFlowProvider>
    );
};

export default PanelBody;
