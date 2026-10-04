import { CSSProperties } from "react";
import { ReactFlow, MiniMap, Controls, Background, BackgroundVariant, SelectionMode, ReactFlowProps, useStore } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import nodeTypes from "../../nodes/NodeTypes";
import ConnectionLine from "../ConnectionLine";
import ZoneFrames from "./ZoneFrames";
import TypedEdge from "./TypedEdge";
import { useHoldCodes } from "../../utils/keymap";

// every edge is drawn in its sockets' colors
const edgeTypes = { default: TypedEdge };

// how far the mouse can move between press and release and still be a click, further is a drag (blender's 3px). the
// same for both, react flow's defaults (a drag past 1px, a click only with no movement at all) left a click that moved a
// pixel as neither, it selected nothing
const CLICK_DISTANCE = 3;

// a theme's paper texture (index.css .canvas-paper), pinned to the graph. it's its own gpu layer that panning only
// slides, so moving the graph never repaints it. the vars live on this element alone so no other styles recalculate
function CanvasPaper() {
    const [x, y, zoom] = useStore((s) => s.transform);
    return <div className="canvas-paper" style={{ "--viewport-x": `${x}px`, "--viewport-y": `${y}px`, "--viewport-zoom": zoom } as CSSProperties} />;
}

// how every node graph looks, with no state of its own. the editor passes its nodes and handlers, the frozen parent
// graph behind an open group passes `frozen` so nothing in it can be touched
function NodeGraphCanvas({ frozen = false, children, ...props }: ReactFlowProps & { frozen?: boolean }) {
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
            {...props}
        >
            <ZoneFrames />
            <CanvasPaper />
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
