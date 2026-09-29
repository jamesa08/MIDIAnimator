import { useEffect, useState } from "react";
import { Handle, NodeResizeControl, Position, useUpdateNodeInternals } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import NodeHeader from "./NodeHeader";
import { socketStyle } from "../utils/sockets";
import { useGroupContext } from "../contexts/GroupContext";
import { NEW_SOCKET } from "../utils/groups";

// window event to rename or remove a socket of the group being edited, the node graph editor applies it.
// detail: { side: "inputs" | "outputs", id, name? } (no name removes it)
export const SOCKET_EDIT_EVENT = "motionkeys:socket-edit";

const handleStyle = {
    width: "14px",
    height: "14px",
    display: "flex",
    justifyContent: "center",
    alignItems: "center",
    position: "absolute" as const,
};

// the group input (side "inputs": the group's inputs come out of it) or group output (side "outputs": the group's
// outputs go into it). connecting to the empty socket at the end adds a socket to the group
function InterfaceNode({ id, data, side }: { id: string; data: any; side: "inputs" | "outputs" }) {
    const { scope, editable } = useGroupContext();
    const updateNodeInternals = useUpdateNodeInternals();
    const preview = data === "preview" || data?.preview === true;
    const sockets = scope?.interface[side] ?? [];
    const [renaming, setRenaming] = useState<string | null>(null);

    // the group's inputs are outputs of this node and the other way around
    const isOutput = side === "inputs";
    const rows = editable && !preview ? [...sockets, { id: NEW_SOCKET, name: "", data_type: "Any" }] : sockets;

    const socketsKey = rows.map((h) => h.id).join(",");
    useEffect(() => {
        if (!preview) updateNodeInternals(id);
    }, [id, socketsKey, preview, updateNodeInternals]);

    const edit = (socket: string, name?: string) => window.dispatchEvent(new CustomEvent(SOCKET_EDIT_EVENT, { detail: { side, id: socket, name } }));

    return (
        <div className={`node${preview ? " preview" : ""}`}>
            <NodeHeader label={side === "inputs" ? "Group Input" : "Group Output"} type="interface" />
            <NodeResizeControl minWidth={160} maxWidth={1000} variant={"line" as any} />
            <div className="node-inner flex flex-col">
                {rows.map((socket) => (
                    <div key={socket.id} className={`node-field field-${isOutput ? "outputs" : "inputs"} interface-socket`} style={{ position: "relative", display: "flex", justifyContent: isOutput ? "flex-end" : "flex-start", gap: 4 }}>
                        {socket.id === NEW_SOCKET ? (
                            <span>&nbsp;</span>
                        ) : renaming === socket.id ? (
                            <input
                                className="nodrag nopan interface-rename"
                                autoFocus
                                defaultValue={socket.name}
                                onBlur={(e) => {
                                    setRenaming(null);
                                    if (e.target.value.trim() && e.target.value !== socket.name) edit(socket.id, e.target.value.trim());
                                }}
                                onKeyDown={(e) => {
                                    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                                    if (e.key === "Escape") setRenaming(null);
                                    e.stopPropagation();
                                }}
                            />
                        ) : (
                            <>
                                {editable && !preview && isOutput && (
                                    <button className="interface-remove nodrag nopan" onClick={() => edit(socket.id)}>
                                        ×
                                    </button>
                                )}
                                <span onDoubleClick={() => editable && !preview && setRenaming(socket.id)}>{socket.name}</span>
                                {editable && !preview && !isOutput && (
                                    <button className="interface-remove nodrag nopan" onClick={() => edit(socket.id)}>
                                        ×
                                    </button>
                                )}
                            </>
                        )}
                        {preview ? (
                            <div className={`react-flow__handle react-flow__handle-${isOutput ? "right" : "left"}`} style={{ ...(isOutput ? { ...handleStyle, right: "-13px" } : { ...handleStyle, left: "-13px" }), ...(socket.id === NEW_SOCKET ? {} : socketStyle(socket.data_type)) }}></div>
                        ) : (
                            <Handle id={socket.id} type={isOutput ? "target" : "source"} position={isOutput ? Position.Right : Position.Left} style={{ ...(isOutput ? { ...handleStyle, right: "-13px" } : { ...handleStyle, left: "-13px" }), ...(socket.id === NEW_SOCKET ? {} : socketStyle(socket.data_type)) }} />
                        )}
                    </div>
                ))}
            </div>
        </div>
    );
}

export default InterfaceNode;
