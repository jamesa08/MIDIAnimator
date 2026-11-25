import { CSSProperties, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { createPortal } from "react-dom";
import { ReactFlow, MiniMap, Controls, Panel, Background, BackgroundVariant, SelectionMode, ReactFlowProps, Edge, EdgeChange, EdgeSelectionChange, InternalNode, Node, NodeSelectionChange, useStore, useStoreApi } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import nodeTypes from "../../nodes/NodeTypes";
import ConnectionLine from "../ConnectionLine";
import ZoneFrames from "./ZoneFrames";
import TypedEdge from "./TypedEdge";
import { addsToSelection, modalOpen, useHoldCodes } from "../../utils/keymap";
import { scrollPans } from "../../utils/scroll";
import { SocketRef } from "../../utils/graphOps";
import { socketPoints } from "./SocketHandle";
import { edgeCrosses, edgePaths } from "./edgeGeometry";
import { boxTakesFrame, isFrame, layoutFrames } from "../../utils/frames";
import { CanvasTool, useCanvasTool } from "../../utils/editorState";
import { useConsoleSettings } from "../../utils/console";

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

// the canvas' dots, react flow draws them closer together as it zooms out. once they'd be under MIN_DOT_SPACING apart
// on screen they're left out: they only read as a faint haze by then, and webkit drawing hundreds of thousands of them
// again on every frame of a pan or zoom cost more than the rest of the graph
const DOT_GAP = 12;
const MIN_DOT_SPACING = 4;
function Dots() {
    const shown = useStore((s) => s.transform[2] * DOT_GAP >= MIN_DOT_SPACING);
    return shown ? <Background variant={BackgroundVariant.Dots} gap={DOT_GAP} size={1} /> : null;
}

// while a box is dragged BoxSelect picks the edges alone (`active`), react flow's own pick, the edges on the nodes inside,
// is dropped. both picking undid the edges the box crosses and put them back on every mouse move, hundreds of edges
// redrawn each time in a big graph. `writing` while BoxSelect's own changes go through
type BoxEdges = { active: boolean; writing: boolean };

// box select takes the edges the box crosses too, not only the ones on the nodes inside it (react flow's), and in a
// graph that has `onBoxSockets` (the editor) the sockets inside it. not the sockets of a node wholly inside it, that node
// is selected. each change goes through the graph's onEdgesChange like react flow's own
function BoxSelect({ nodes, edges, onBoxSockets, owns }: { nodes?: Node[]; edges?: Edge[]; onBoxSockets?: (sockets: SocketRef[]) => void; owns: BoxEdges }) {
    const store = useStoreApi();
    const selectionRect = useStore((s) => s.userSelectionRect);
    const selecting = useStore((s) => s.userSelectionActive);
    const latest = useRef({ nodes, edges, onBoxSockets });
    latest.current = { nodes, edges, onBoxSockets };
    useEffect(() => {
        const { userSelectionActive, transform, domNode, nodeLookup, triggerEdgeChanges } = store.getState();
        // not yet dragged, a press on the graph sets an empty box
        owns.active = !!selectionRect && userSelectionActive;
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
        const paths = edgePaths(domNode);
        const changes: EdgeSelectionChange[] = [];
        for (const edge of edges ?? []) {
            const selected = inside.has(edge.source) || inside.has(edge.target) || edgeCrosses(paths.get(edge.id), box);
            if (selected !== !!edge.selected) changes.push({ id: edge.id, type: "select", selected });
        }
        if (changes.length > 0) {
            owns.writing = true;
            try {
                triggerEdgeChanges(changes);
            } finally {
                owns.writing = false;
            }
        }
    }, [selectionRect, store, owns]);

    // react flow leaves frames out (they aren't selectable to it), the box takes one like blender (boxTakesFrame). before
    // the box is drawn, react flow's own selection deselects them as it goes
    useLayoutEffect(() => {
        const { userSelectionActive, transform, nodeLookup, triggerNodeChanges } = store.getState();
        if (!selectionRect || !userSelectionActive) return;
        const [tx, ty, zoom] = transform;
        const box = { x: (selectionRect.x - tx) / zoom, y: (selectionRect.y - ty) / zoom, width: selectionRect.width / zoom, height: selectionRect.height / zoom };
        const changes: NodeSelectionChange[] = [];
        for (const node of nodeLookup.values()) {
            if (!isFrame(node)) continue;
            const { x, y } = node.internals.positionAbsolute;
            const selected = boxTakesFrame(node, { x, y, width: node.measured.width ?? 0, height: node.measured.height ?? 0 }, box);
            if (selected !== !!node.selected) changes.push({ id: node.id, type: "select", selected });
        }
        if (changes.length > 0) triggerNodeChanges(changes);
    });

    // the box itself, drawn on the body over the docked panels (react flow's own is hidden, index.css). it stays inside
    // the canvas and under menus and modals
    const canvas = selectionRect && selecting ? store.getState().domNode?.getBoundingClientRect() : undefined;
    if (!selectionRect || !canvas) return null;
    return createPortal(
        <div className="box-select-layer" style={{ left: canvas.left, top: canvas.top, width: canvas.width, height: canvas.height }}>
            <div className="box-select-rect" style={{ width: selectionRect.width, height: selectionRect.height, transform: `translate(${selectionRect.x}px, ${selectionRect.y}px)` }} />
        </div>,
        document.body
    );
}

// the frame rate over the last half second and the slowest frame in it, a hitch the rate alone would hide, shown
// fresh every frame. under the minimap while the console's FPS counter setting is on, it only counts while it's shown
function FpsCounter() {
    const { fps_counter } = useConsoleSettings();
    return fps_counter ? <FpsReadout /> : null;
}

// how far back the rate and the slowest frame look
const FPS_WINDOW_MS = 500;

function FpsReadout() {
    const fpsRef = useRef<HTMLSpanElement>(null);
    const worstRef = useRef<HTMLSpanElement>(null);
    useEffect(() => {
        let request = 0;
        // the recent frames' times, oldest first
        const times: number[] = [];
        // written straight into the text every frame, a render each frame would cost more than it measures
        const tick = (now: number) => {
            times.push(now);
            while (times.length > 2 && now - times[0] > FPS_WINDOW_MS) times.shift();
            if (times.length > 1) {
                let worst = 0;
                for (let i = 1; i < times.length; i++) worst = Math.max(worst, times[i] - times[i - 1]);
                if (fpsRef.current) fpsRef.current.textContent = `${Math.round(((times.length - 1) * 1000) / (now - times[0]))} fps`;
                if (worstRef.current) worstRef.current.textContent = `${Math.round(worst)} ms`;
            }
            request = requestAnimationFrame(tick);
        };
        request = requestAnimationFrame(tick);
        return () => cancelAnimationFrame(request);
    }, []);
    return (
        <Panel position="top-right" className="fps-counter">
            <span ref={fpsRef} />
            <span ref={worstRef} />
        </Panel>
    );
}

// the wheel over a graph. react flow pans for a scroll, this takes the scrolls that zoom instead: a mouse wheel
// (scrollPans), cmd with a swipe like blender's ctrl, and a trackpad pinch (macos sends it as a wheel event with ctrl).
// the zooms asked for since the last frame are put together and drawn once in the next one: a pinch sends several wheel
// events a frame, and react flow's own pinch redrew the graph for each of them. nothing moves while a modal takes the
// input
function WheelNavigation() {
    const store = useStoreApi();
    const domNode = useStore((s) => s.domNode);
    useEffect(() => {
        if (!domNode) return;
        // the zoom asked for since the last frame, as a factor, around the last event's point
        let pending: { factor: number; x: number; y: number } | null = null;
        let request = 0;
        const zoomBy = (x: number, y: number, factor: number) => {
            pending = { factor: (pending?.factor ?? 1) * factor, x, y };
            if (request) return;
            request = requestAnimationFrame(() => {
                request = 0;
                const zoom = pending!;
                pending = null;
                zoomAround(store, zoom.x, zoom.y, store.getState().transform[2] * zoom.factor);
            });
        };
        const handleWheel = (event: WheelEvent) => {
            // only over the graph itself like react flow's (not the controls or minimap), and not over a part that
            // scrolls itself (a viewer's text)
            const target = event.target as Element;
            if (!target.closest?.(".react-flow__renderer") || target.closest(".nowheel")) return;
            const stopped = modalOpen();
            const pinch = event.ctrlKey && !event.metaKey;
            if (!stopped && !pinch && !event.metaKey && scrollPans(event)) return;
            // react flow never sees it
            event.preventDefault();
            event.stopPropagation();
            if (stopped) return;

            // zoom around the cursor, as fast as react flow's zoom on scroll, and a pinch as fast as its pinch
            const speed = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? 0.05 : event.deltaMode === WheelEvent.DOM_DELTA_PAGE ? 1 : 0.002;
            const rect = domNode.getBoundingClientRect();
            zoomBy(event.clientX - rect.left, event.clientY - rect.top, 2 ** (-event.deltaY * speed * (pinch ? PINCH_SPEED : 1)));
        };
        // capture, so it runs before react flow's
        domNode.addEventListener("wheel", handleWheel, { capture: true, passive: false });
        return () => {
            domNode.removeEventListener("wheel", handleWheel, { capture: true });
            cancelAnimationFrame(request);
        };
    }, [domNode, store]);
    return null;
}

// how much faster a pinch zooms than a scroll, react flow's
const PINCH_SPEED = 10;

// the view zoomed to `zoom` (kept within the canvas's limits) with the point (px, py) on the canvas staying put
function zoomAround(store: ReturnType<typeof useStoreApi>, px: number, py: number, zoom: number) {
    const { transform, minZoom, maxZoom, panZoom } = store.getState();
    const [x, y, current] = transform;
    const next = Math.min(maxZoom, Math.max(minZoom, zoom));
    panZoom?.setViewport({ x: px - ((px - x) * next) / current, y: py - ((py - y) * next) / current, zoom: next });
}

// the zoom tool: a left drag zooms around where it was pressed, up zooms in and down out like blender's ctrl+middle drag
function ZoomDrag() {
    const store = useStoreApi();
    const domNode = useStore((s) => s.domNode);
    useEffect(() => {
        if (!domNode) return;
        const handleDown = (event: PointerEvent) => {
            const pane = (event.target as Element).closest(".react-flow__pane");
            if (event.button !== 0 || !pane || modalOpen()) return;
            const rect = domNode.getBoundingClientRect();
            const [px, py] = [event.clientX - rect.left, event.clientY - rect.top];
            const [startY, startZoom] = [event.clientY, store.getState().transform[2]];
            pane.classList.add("zooming");
            const move = (moved: PointerEvent) => zoomAround(store, px, py, startZoom * 2 ** ((startY - moved.clientY) * 0.01));
            const end = () => {
                pane.classList.remove("zooming");
                window.removeEventListener("pointermove", move);
                window.removeEventListener("pointerup", end);
                window.removeEventListener("blur", end);
            };
            window.addEventListener("pointermove", move);
            window.addEventListener("pointerup", end);
            window.addEventListener("blur", end);
        };
        domNode.addEventListener("pointerdown", handleDown);
        return () => domNode.removeEventListener("pointerdown", handleDown);
    }, [domNode, store]);
    return null;
}

// multi select for a graph (the editor, a note map) while shift or cmd (ctrl off a mac) is held, tracked here instead
// of multiSelectionKeyCode. react flow ignores a keyup inside an input, so releasing shift after shift+a focused the
// add menu search left multi select stuck on (clicking another node added to the selection).
// reading the modifiers off every key/mouse event (addsToSelection) means it can't get stuck
export function useMultiSelection() {
    const store = useStoreApi();
    useEffect(() => {
        const setMultiSelection = (active: boolean) => {
            if (store.getState().multiSelectionActive !== active) {
                store.setState({ multiSelectionActive: active });
            }
        };
        const handleEvent = (event: KeyboardEvent | MouseEvent) => setMultiSelection(addsToSelection(event));
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

// a click on the graph does nothing while the pan or zoom tool is on, react flow's pane would clear the selection. the
// minimap and the zoom buttons still take theirs
function stopGraphClick(event: React.MouseEvent) {
    if (!(event.target as Element).closest(".react-flow__pane")) return;
    event.preventDefault();
    event.stopPropagation();
}

// what each tool changes on react flow. pan and zoom keep clicks from the graph, box select's click is the pane's (it clears
// the selection, or keeps a frame's as a click inside it does)
const stopClicks = { onClickCapture: stopGraphClick, onContextMenuCapture: stopGraphClick };
const TOOL_PROPS: Record<Exclude<CanvasTool, "select">, Partial<ReactFlowProps>> = {
    box_select: {},
    pan: { panOnDrag: true, selectionKeyCode: null, ...stopClicks },
    zoom: { selectionOnDrag: false, selectionKeyCode: null, ...stopClicks },
};

// how every node graph looks, with no state of its own. the editor passes its nodes and handlers, the frozen parent
// graph behind an open group passes `frozen` so nothing in it can be touched. `onBoxSockets` gets the sockets a box
// selects while it's dragged (BoxSelect)
function NodeGraphCanvas({ frozen = false, children, nodes, edges, onBoxSockets, ...props }: ReactFlowProps & { frozen?: boolean; onBoxSockets?: (sockets: SocketRef[]) => void }) {
    // a connection made by a signal tag is drawn as the tags on its sockets (SocketTag), not as a wire
    const shown = useMemo(() => edges?.map((edge: any) => (edge.tagged && !edge.hidden ? { ...edge, hidden: true } : edge)), [edges]);
    // frames fitted around what's in them by the sizes on screen, the frozen graph has none and draws them where they were saved.
    // a collapsed node is as tall as its header whatever height it was given
    const shownNodes = useMemo(
        () =>
            nodes &&
            layoutFrames(
                nodes.map((node: any) => (node.data?.collapsed && node.height !== undefined ? { ...node, height: undefined } : node)),
                !frozen
            ),
        [nodes, frozen]
    );
    // react flow's own keys are off, the keymap runs them (src/utils/keymap.ts). box select is a key held while dragging,
    // react flow tracks that one itself
    const boxSelectKeys = useHoldCodes("node_editor", "box_select");
    // a tool other than select takes the left drag anywhere, over nodes too, and nothing can be moved or wired. what's
    // drawn lets the mouse through to the pane (index.css .canvas-tool), so the tool's cursor shows over nodes, sockets
    // and edges alike. box select draws react flow's own box from there, a held pan (alt) still pans with any tool
    const picked = useCanvasTool();
    const tool = frozen ? "select" : picked;
    const panHeld = props.panOnDrag === true;
    const boxEdges = useRef<BoxEdges>({ active: false, writing: false }).current;
    const { onEdgesChange } = props;
    const edgesChange = useMemo(() => onEdgesChange && ((changes: EdgeChange[]) => onEdgesChange(boxEdges.active && !boxEdges.writing ? changes.filter((c) => c.type !== "select") : changes)), [onEdgesChange, boxEdges]);
    const toolProps = tool === "select" ? {} : { ...TOOL_PROPS[tool], className: `${props.className ?? ""} canvas-tool tool-${tool}` };
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
            panOnScroll={!frozen}
            panOnScrollSpeed={1}
            zoomActivationKeyCode={null}
            nodes={shownNodes}
            edges={shown}
            {...props}
            onEdgesChange={edgesChange}
            {...toolProps}
        >
            <ZoneFrames />
            <CanvasPaper />
            {!frozen && <BoxSelect nodes={shownNodes} edges={shown} onBoxSockets={onBoxSockets} owns={boxEdges} />}
            {!frozen && <WheelNavigation />}
            {tool === "zoom" && !panHeld && <ZoomDrag />}
            {!frozen && (
                <>
                    <Dots />
                    <Controls />
                    <MiniMap position="top-right" style={{ width: 100, height: 75 }} />
                    <FpsCounter />
                </>
            )}
            {frozen && <Dots />}
            {children}
        </ReactFlow>
    );
}

export default NodeGraphCanvas;
