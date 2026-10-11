import { CSSProperties, useEffect, useMemo, useRef } from "react";
import { ReactFlow, MiniMap, Controls, Background, BackgroundVariant, SelectionMode, ReactFlowProps, Edge, EdgeSelectionChange, InternalNode, Node, useStore, useStoreApi } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import nodeTypes from "../../nodes/NodeTypes";
import ConnectionLine from "../ConnectionLine";
import ZoneFrames from "./ZoneFrames";
import TypedEdge from "./TypedEdge";
import { useHoldCodes } from "../../utils/keymap";
import { SocketRef } from "../../utils/graphOps";
import { socketPoints } from "./SocketHandle";

// every edge is drawn in its sockets' colors
const edgeTypes = { default: TypedEdge };

// how far the mouse can move between press and release and still be a click, further is a drag (blender's 3px). the
// same for both, react flow's defaults (a drag past 1px, a click only with no movement at all) left a click that moved a
// pixel as neither, it selected nothing
export const CLICK_DISTANCE = 3;

// a theme's paper texture (index.css .canvas-paper), pinned to the graph. it's its own gpu layer that panning only
// slides, so moving the graph never repaints it. the vars live on this element alone so no other styles recalculate
function CanvasPaper() {
    const [x, y, zoom] = useStore((s) => s.transform);
    return <div className="canvas-paper" style={{ "--viewport-x": `${x}px`, "--viewport-y": `${y}px`, "--viewport-zoom": zoom } as CSSProperties} />;
}

// whether an edge's path passes through a box (flow coordinates), sampled along the path as it's drawn
function edgeCrosses(root: Element | null | undefined, id: string, box: { x0: number; y0: number; x1: number; y1: number }): boolean {
    const path = root?.querySelector<SVGPathElement>(`.react-flow__edge[data-id="${CSS.escape(id)}"] .react-flow__edge-path`);
    if (!path) return false;
    // nowhere near the box, no need to walk it
    const bounds = path.getBBox();
    if (bounds.x > box.x1 || bounds.x + bounds.width < box.x0 || bounds.y > box.y1 || bounds.y + bounds.height < box.y0) return false;
    const length = path.getTotalLength();
    for (let at = 0; at <= length; at += 4) {
        const p = path.getPointAtLength(at);
        if (p.x >= box.x0 && p.x <= box.x1 && p.y >= box.y0 && p.y <= box.y1) return true;
    }
    return false;
}

// box select takes the edges the box crosses too, not only the ones on the nodes inside it (react flow's), and in a
// graph that has `onBoxSockets` (the editor) the sockets inside it. not the sockets of a node wholly inside it, that node
// is selected. each change goes through the graph's onEdgesChange like react flow's own
function BoxSelect({ nodes, edges, onBoxSockets }: { nodes?: Node[]; edges?: Edge[]; onBoxSockets?: (sockets: SocketRef[]) => void }) {
    const store = useStoreApi();
    const selectionRect = useStore((s) => s.userSelectionRect);
    const latest = useRef({ nodes, edges, onBoxSockets });
    latest.current = { nodes, edges, onBoxSockets };
    useEffect(() => {
        const { userSelectionActive, transform, domNode, nodeLookup, triggerEdgeChanges } = store.getState();
        // not yet dragged, a press on the graph sets an empty box
        if (!selectionRect || !userSelectionActive) return;
        const { nodes, edges, onBoxSockets } = latest.current;
        const [tx, ty, zoom] = transform;
        const box = { x0: (selectionRect.x - tx) / zoom, y0: (selectionRect.y - ty) / zoom, x1: (selectionRect.x + selectionRect.width - tx) / zoom, y1: (selectionRect.y + selectionRect.height - ty) / zoom };
        const inBox = (x: number, y: number) => x >= box.x0 && x <= box.x1 && y >= box.y0 && y <= box.y1;

        if (onBoxSockets) {
            const whole = (n: InternalNode) => {
                const { x, y } = n.internals.positionAbsolute;
                const [width, height] = [n.measured.width ?? 0, n.measured.height ?? 0];
                return width > 0 && inBox(x, y) && inBox(x + width, y + height);
            };
            const points = [...nodeLookup.values()].filter((n) => !n.hidden && !whole(n)).flatMap(socketPoints);
            onBoxSockets(points.filter((p) => inBox(p.x, p.y)).map(({ node, side, socket }) => ({ node, side, socket })));
        }

        const inside = new Set((nodes ?? []).filter((n) => n.selected).map((n) => n.id));
        const changes: EdgeSelectionChange[] = [];
        for (const edge of edges ?? []) {
            const selected = inside.has(edge.source) || inside.has(edge.target) || edgeCrosses(domNode, edge.id, box);
            if (selected !== !!edge.selected) changes.push({ id: edge.id, type: "select", selected });
        }
        if (changes.length > 0) triggerEdgeChanges(changes);
    }, [selectionRect, store]);
    return null;
}

// shift multi select for a graph (the editor, a note map), tracked here instead of multiSelectionKeyCode.
// react flow ignores a keyup inside an input, so releasing shift after shift+a focused the
// add menu search left multi select stuck on (clicking another node added to the selection).
// reading shiftKey off every key/mouse event means it can't get stuck
export function useShiftMultiSelection() {
    const store = useStoreApi();
    useEffect(() => {
        const setMultiSelection = (active: boolean) => {
            if (store.getState().multiSelectionActive !== active) {
                store.setState({ multiSelectionActive: active });
            }
        };
        const handleEvent = (event: KeyboardEvent | MouseEvent) => setMultiSelection(event.shiftKey);
        const handleBlur = () => setMultiSelection(false);

        // capture phase so it's set before react flow handles the click
        window.addEventListener("keydown", handleEvent, true);
        window.addEventListener("keyup", handleEvent, true);
        window.addEventListener("pointerdown", handleEvent, true);
        window.addEventListener("mousedown", handleEvent, true);
        window.addEventListener("blur", handleBlur);
        return () => {
            window.removeEventListener("keydown", handleEvent, true);
            window.removeEventListener("keyup", handleEvent, true);
            window.removeEventListener("pointerdown", handleEvent, true);
            window.removeEventListener("mousedown", handleEvent, true);
            window.removeEventListener("blur", handleBlur);
        };
    }, [store]);
}

// how every node graph looks, with no state of its own. the editor passes its nodes and handlers, the frozen parent
// graph behind an open group passes `frozen` so nothing in it can be touched. `onBoxSockets` gets the sockets a box
// selects while it's dragged (BoxSelect)
function NodeGraphCanvas({ frozen = false, children, edges, onBoxSockets, ...props }: ReactFlowProps & { frozen?: boolean; onBoxSockets?: (sockets: SocketRef[]) => void }) {
    // a connection made by a signal tag is drawn as the tags on its sockets (SocketTag), not as a wire
    const shown = useMemo(() => edges?.map((edge: any) => (edge.tagged && !edge.hidden ? { ...edge, hidden: true } : edge)), [edges]);
    // react flow's own keys are off, the keymap runs them (src/utils/keymap.ts). box select is a key held while dragging,
    // react flow tracks that one itself
    const boxSelectKeys = useHoldCodes("node_editor", "box_select");
    return (
        <ReactFlow
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            connectionLineComponent={ConnectionLine}
            selectionOnDrag={!frozen}
            multiSelectionKeyCode={null}
            selectionKeyCode={frozen || boxSelectKeys.length === 0 ? null : boxSelectKeys}
            deleteKeyCode={null}
            connectOnClick={false}
            nodeDragThreshold={CLICK_DISTANCE}
            nodeClickDistance={CLICK_DISTANCE}
            paneClickDistance={CLICK_DISTANCE}
            panActivationKeyCode={null}
            selectionMode={SelectionMode.Partial}
            minZoom={0.05}
            nodesDraggable={!frozen}
            nodesConnectable={!frozen}
            elementsSelectable={!frozen}
            panOnDrag={!frozen}
            zoomOnScroll={!frozen}
            zoomOnPinch={!frozen}
            edges={shown}
            {...props}
        >
            <ZoneFrames />
            <CanvasPaper />
            {!frozen && <BoxSelect nodes={props.nodes} edges={shown} onBoxSockets={onBoxSockets} />}
            {!frozen && (
                <>
                    <Background variant={BackgroundVariant.Dots} gap={12} size={1} />
                    <Controls />
                    <MiniMap position="top-right" style={{ width: 100, height: 75 }} />
                </>
            )}
            {frozen && <Background variant={BackgroundVariant.Dots} gap={12} size={1} />}
            {children}
        </ReactFlow>
    );
}

export default NodeGraphCanvas;
