import { CSSProperties, useId, useMemo, useState } from "react";
import { ConnectionLineComponentProps, getSimpleBezierPath, InternalNode, Position, useStore } from "@xyflow/react";
import { useGroupContext } from "../contexts/GroupContext";
import { SOCKET_COLORS } from "../styles";
import { SocketRef } from "../utils/graphOps";
import { inputHandle, outputHandle, specLookup } from "../utils/groups";
import { useNodeSpecs } from "../utils/nodeEntries";
import { socketCategory } from "../utils/sockets";
import { draggedAlong, handleSide } from "./nodegraph/SocketHandle";
import { connectAlong } from "./nodegraph/connectAlong";

// one link being dragged, in its sockets' colors like a connected edge (TypedEdge): fading from `start`'s color to `end`'s
function Wire({ x, y, endX, endY, start, end, opacity }: { x: number; y: number; endX: number; endY: number; start: string; end: string; opacity?: number }) {
    const gradientId = `wire-gradient-${useId()}`;
    const [d] = getSimpleBezierPath({ sourceX: x, sourceY: y, sourcePosition: Position.Right, targetX: endX, targetY: endY, targetPosition: Position.Left });
    return (
        <>
            {start !== end && (
                <defs>
                    <linearGradient id={gradientId} gradientUnits="userSpaceOnUse" x1={x} y1={y} x2={endX} y2={endY}>
                        <stop offset="0%" style={{ stopColor: start }} />
                        <stop offset="100%" style={{ stopColor: end }} />
                    </linearGradient>
                </defs>
            )}
            <path className="node-edge" fill="none" strokeWidth={3} opacity={opacity} style={{ stroke: start === end ? start : `url(#${gradientId})` }} d={d} />
        </>
    );
}

export default ({ fromX, fromY, toX, toY, fromNode, fromHandle, toNode, toHandle, connectionStatus, connectionLineStyle }: ConnectionLineComponentProps) => {
    const [isHovering, setIsHovering] = useState(true);

    // the other selected sockets come along. over a socket the link can connect to, each goes into the socket it will
    // connect to (connectAlong) and the ones that won't connect aren't drawn. anywhere else they end as far from the
    // cursor as they are from the dragged socket
    const nodeLookup = useStore((s) => s.nodeLookup);
    const edges = useStore((s) => s.edges);
    const fromSide = fromHandle ? handleSide(fromHandle.type) : "inputs";
    const along = useMemo(() => (fromNode && fromHandle?.id ? draggedAlong(nodeLookup, { node: fromNode.id, side: fromSide, socket: fromHandle.id }) : []), [nodeLookup, fromNode?.id, fromHandle?.id, fromSide]);
    const dropOn = connectionStatus === "valid" && toNode && toHandle?.id ? { node: toNode.id, side: handleSide(toHandle.type), socket: toHandle.id } : null;

    // a socket's color from its type, a graph whose nodes have no specs (a note map) gives its own as the line's stroke
    const specs = useNodeSpecs();
    const { groups, scope } = useGroupContext();
    const lookup = useMemo(() => specLookup(Object.fromEntries(specs.map((spec: any) => [spec.id, spec])), groups), [specs, groups]);

    const links = useMemo(() => {
        if (!dropOn || !fromNode || !fromHandle?.id || along.length === 0) return [];
        const from = { node: fromNode.id, side: fromSide, socket: fromHandle.id };
        const [output, input] = fromSide === "outputs" ? [from, dropOn] : [dropOn, from];
        // outputs dropped on a multi input all go into it
        const inputNode = nodeLookup.get(input.node);
        const multi = fromSide === "outputs" && !!inputNode && !!inputHandle(lookup, inputNode, input.socket, scope).multi;
        return connectAlong(nodeLookup, edges, output, input, along, multi);
    }, [dropOn?.node, dropOn?.socket, along, nodeLookup, edges, lookup, scope]);

    const given = (connectionLineStyle as CSSProperties | undefined)?.stroke;
    const colorOf = (node: InternalNode | null | undefined, socket: SocketRef | null) => {
        if (given) return given;
        if (!node || !socket) return SOCKET_COLORS.any;
        return SOCKET_COLORS[socketCategory((socket.side === "inputs" ? inputHandle : outputHandle)(lookup, node, socket.socket, scope).data_type)];
    };
    const fromColor = colorOf(fromNode, fromHandle?.id ? { node: fromNode.id, side: fromSide, socket: fromHandle.id } : null);
    const dropColor = dropOn ? colorOf(toNode, dropOn) : fromColor;

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
                const color = colorOf(nodeLookup.get(socket.node), socket);
                return <Wire key={`${socket.node}\n${socket.socket}`} x={socket.x} y={socket.y} endX={end.x} endY={end.y} start={color} end={target ? colorOf(nodeLookup.get(target.node), target) : color} opacity={target ? 1 : 0.5} />;
            })}
            <Wire x={fromX} y={fromY} endX={toX} endY={toY} start={fromColor} end={dropColor} />
            <circle cx={toX} cy={toY} fill="#fff" r={3} stroke={"black"} strokeWidth={1.5} />
            {/* dropped on nothing, a single link opens the add menu */}
            {!isHovering && along.length === 0 && <path className="edge-plus-sign" stroke={"black"} d={"M0,-5 V5 M-5,0 H5"} style={{ transform: `translate(${toX + 15}px, ${toY - 15}px)` }} />}
        </g>
    );
};
