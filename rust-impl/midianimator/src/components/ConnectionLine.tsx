import { useState } from "react";
import { Connection, ConnectionState, getSimpleBezierPath, Position, useConnection, useReactFlow } from "@xyflow/react";

export default ({ fromX, fromY, toX, toY }: { fromX: number; fromY: number; toX: number; toY: number }) => {
    const [d] = getSimpleBezierPath({
        sourceX: fromX,
        sourceY: fromY,
        sourcePosition: Position.Right,
        targetX: toX,
        targetY: toY,
        targetPosition: Position.Left,
    });
    const [isHovering, setIsHovering] = useState(true);

    // FIXME would like a better way to detect if the user is hovering over a handle, dirty hack
    // the reason for the timeout is that the handle is not rendered immediately, and we need to wait for it to be rendered
    // which is why its not a great solution
    setTimeout(() => setIsHovering(document.querySelectorAll(".react-flow__handle.connectingto").length > 0), 10);

    return (
        <g>
            <path fill="none" stroke={"black"} strokeWidth={1.5} className="node-edge" d={d} />
            <circle cx={toX} cy={toY} fill="#fff" r={3} stroke={"black"} strokeWidth={1.5} />
            {!isHovering && <path className="edge-plus-sign" stroke={"black"} d={"M0,-5 V5 M-5,0 H5"} style={{ transform: `translate(${toX + 15}px, ${toY - 15}px)` }} />}
        </g>
    );
};
