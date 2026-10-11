import { CSSProperties, useCallback, useEffect, useSyncExternalStore } from "react";
import { ReactFlowState, useNodeId, useReactFlow, useStore } from "@xyflow/react";
import { useGroupContext } from "../../contexts/GroupContext";
import { SOCKET_COLORS } from "../../styles";
import { socketCategory } from "../../utils/sockets";
import { useBadInput } from "./ErrorBadge";
import { editTag, selectSocket, useSocketSelected } from "./SocketHandle";

// signal tags (src-tauri/src/graph/tags.rs): a name on a socket that connects it without a wire. the connection is a
// stored edge marked `tagged`, drawn as a tag next to each socket instead (NodeGraphCanvas hides the edge)

// the tag under the mouse as "scope\nname", every tag with that name in that graph lights up with it
let hovered: string | null = null;
const listeners = new Set<() => void>();
const setHovered = (next: string | null) => {
    if (hovered === next) return;
    hovered = next;
    listeners.forEach((listener) => listener());
};
const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
};

// the tag on a socket, beside it outside the node. dashed while it connects nothing (broken). a click selects its socket,
// deleting that removes the tag
function SocketTag({ side, socket, name, dataType }: { side: "inputs" | "outputs"; socket: string; name: string; dataType?: string }) {
    const nodeId = useNodeId() ?? "";
    const { scopeId } = useGroupContext();
    const { getInternalNode, setCenter, getZoom } = useReactFlow();
    const key = `${scopeId ?? ""}\n${name}`;
    const lit = useSyncExternalStore(subscribe, () => hovered === key);
    // a tag going away under the mouse (its node deleted) doesn't leave the others lit
    useEffect(() => () => setHovered(hovered === key ? null : hovered), [key]);
    const bad = useBadInput(nodeId, side === "inputs" ? socket : null);

    // stored edges are reversed, `source`/`sourceHandle` is the node taking the value and its input. an input's tag
    // gives the node it takes its value from, an output's whether anything uses it
    const connection = useCallback(
        (s: ReactFlowState) => {
            if (side === "inputs") return s.edges.find((e: any) => e.tagged && e.source === nodeId && e.sourceHandle === socket)?.target ?? "";
            return s.edges.some((e: any) => e.tagged && e.target === nodeId && e.targetHandle === socket) ? nodeId : "";
        },
        [side, nodeId, socket]
    );
    const connectedTo = useStore(connection);
    const selected = useSocketSelected(nodeId, side, socket);
    const select = (event: React.MouseEvent) => selectSocket(scopeId, nodeId, side, socket, event);

    // an input's goes to the output it takes its value from, any other opens the tag menu
    const doubleClick = (event: React.MouseEvent) => {
        const source = side === "inputs" && connectedTo ? getInternalNode(connectedTo) : undefined;
        if (!source) return editTag(scopeId, nodeId, side, socket, event);
        event.stopPropagation();
        const { x, y } = source.internals.positionAbsolute;
        setCenter(x + (source.measured.width ?? 0) / 2, y + (source.measured.height ?? 0) / 2, { zoom: getZoom(), duration: 200 });
    };

    const color = SOCKET_COLORS[socketCategory(dataType ?? "Any")];
    return (
        <div className={`socket-tag socket-tag-${side} nodrag${connectedTo ? "" : " broken"}${lit ? " lit" : ""}${bad ? " bad" : ""}${selected ? " selected" : ""}`} style={{ "--socket-color": color } as CSSProperties} onMouseEnter={() => setHovered(key)} onMouseLeave={() => setHovered(null)} onClick={select} onDoubleClick={doubleClick}>
            <span className="socket-tag-name">{name}</span>
        </div>
    );
}

export default SocketTag;
