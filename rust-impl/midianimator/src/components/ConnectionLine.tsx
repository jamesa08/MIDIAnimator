import { getSimpleBezierPath, Position, useConnection } from "@xyflow/react";

export default ({ fromX, fromY, toX, toY }: { fromX: number; fromY: number; toX: number; toY: number }) => {
    const [d] = getSimpleBezierPath({
        sourceX: fromX,
        sourceY: fromY,
        sourcePosition: Position.Right,
        targetX: toX,
        targetY: toY,
        targetPosition: Position.Left,
    });

    return (
        <g>
            <path fill="none" stroke={"black"} strokeWidth={1.5} className="node-edge" d={d} />
            <circle cx={toX} cy={toY} fill="#fff" r={3} stroke={"black"} strokeWidth={1.5} />
            <path className="edge-plus-sign" r={3} stroke={"black"} d={"M0,-5 V5 M-5,0 H5"} style={{ transform: `translate(${toX + 15}px, ${toY - 15}px)` }} />
        </g>
    );
};
