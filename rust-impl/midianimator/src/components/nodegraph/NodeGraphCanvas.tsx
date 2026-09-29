import { ReactFlow, MiniMap, Controls, Background, BackgroundVariant, SelectionMode, ReactFlowProps } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import nodeTypes from "../../nodes/NodeTypes";
import ConnectionLine from "../ConnectionLine";
import ZoneFrames from "./ZoneFrames";
import TypedEdge from "./TypedEdge";

// every edge is drawn in its sockets' colors
const edgeTypes = { default: TypedEdge };

// how every node graph looks, with no state of its own. the editor passes its nodes and handlers, the frozen parent
// graph behind an open group passes `frozen` so nothing in it can be touched
function NodeGraphCanvas({ frozen = false, children, ...props }: ReactFlowProps & { frozen?: boolean }) {
    return (
        <ReactFlow
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            connectionLineComponent={ConnectionLine}
            selectionOnDrag={!frozen}
            multiSelectionKeyCode={null}
            selectionKeyCode={frozen ? null : "b"}
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
