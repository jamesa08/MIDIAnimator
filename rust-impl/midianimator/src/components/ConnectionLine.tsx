import { useMemo, useState } from "react";
import { ConnectionLineComponentProps, getSimpleBezierPath, Position, useStore } from "@xyflow/react";
import { draggedAlong, handleSide } from "./nodegraph/SocketHandle";
import { connectAlong } from "./nodegraph/connectAlong";

export default ({ fromX, fromY, toX, toY, fromNode, fromHandle, toNode, toHandle, connectionStatus }: ConnectionLineComponentProps) => {
    const path = (x: number, y: number, endX: number, endY: number) =>
        getSimpleBezierPath({
            sourceX: x,
            sourceY: y,
            sourcePosition: Position.Right,
            targetX: endX,
            targetY: endY,
            targetPosition: Position.Left,
        })[0];
    const [isHovering, setIsHovering] = useState(true);

    // the other selected sockets come along. over a socket the link can connect to, each goes into the socket it will
    // connect to (connectAlong) and the ones that won't connect aren't drawn. anywhere else they end as far from the
    // cursor as they are from the dragged socket
    const nodeLookup = useStore((s) => s.nodeLookup);
    const edges = useStore((s) => s.edges);
    const fromSide = fromHandle ? handleSide(fromHandle.type) : "inputs";
    const along = useMemo(() => (fromNode && fromHandle?.id ? draggedAlong(nodeLookup, { node: fromNode.id, side: fromSide, socket: fromHandle.id }) : []), [nodeLookup, fromNode?.id, fromHandle?.id, fromSide]);
    const dropOn = connectionStatus === "valid" && toNode && toHandle?.id ? { node: toNode.id, side: handleSide(toHandle.type), socket: toHandle.id } : null;
    const links = useMemo(() => {
        if (!dropOn || !fromNode || !fromHandle?.id || along.length === 0) return [];
        const from = { node: fromNode.id, side: fromSide, socket: fromHandle.id };
        const [output, input] = fromSide === "outputs" ? [from, dropOn] : [dropOn, from];
        return connectAlong(nodeLookup, edges, output, input, along);
    }, [dropOn?.node, dropOn?.socket, along, nodeLookup, edges]);

    // FIXME would like a better way to detect if the user is hovering over a handle, dirty hack
    // the reason for the timeout is that the handle is not rendered immediately, and we need to wait for it to be rendered
    // which is why its not a great solution
    setTimeout(() => setIsHovering(document.querySelectorAll(".react-flow__handle.connectingto").length > 0), 10);

    return (
        <g>
            {along.map((socket) => {
                const target = links.find((link) => link.socket.node === socket.node && link.socket.socket === socket.socket)?.target;
                if (dropOn && !target) return null;
                const end = target ?? { x: toX + socket.x - fromX, y: toY + socket.y - fromY };
                return <path key={`${socket.node}\n${socket.socket}`} fill="none" stroke={"black"} strokeWidth={3} opacity={target ? 1 : 0.5} className="node-edge" d={path(socket.x, socket.y, end.x, end.y)} />;
            })}
            <path fill="none" stroke={"black"} strokeWidth={3} className="node-edge" d={path(fromX, fromY, toX, toY)} />
            <circle cx={toX} cy={toY} fill="#fff" r={3} stroke={"black"} strokeWidth={1.5} />
            {/* dropped on nothing, a single link opens the add menu */}
            {!isHovering && along.length === 0 && <path className="edge-plus-sign" stroke={"black"} d={"M0,-5 V5 M-5,0 H5"} style={{ transform: `translate(${toX + 15}px, ${toY - 15}px)` }} />}
        </g>
    );
};
