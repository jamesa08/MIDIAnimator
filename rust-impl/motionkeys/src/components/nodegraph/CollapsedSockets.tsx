import { Position } from "@xyflow/react";
import { socketStyle } from "../../utils/sockets";
import SocketHandle from "./SocketHandle";

// a collapsed node (Blender's hide) is its header with every socket it draws spread down its ends, the header grows
// when a side has more than fit beside its name

// a socket's size and how far apart they are down an end, and the room above and below them
const SIZE = 10;
const STEP = 10;
const PAD = 8;

type Socket = { id: string; data_type: string };

// how tall a collapsed node with these sockets is at least
export const collapsedHeight = (inputs: Socket[], outputs: Socket[]) => Math.max(inputs.length, outputs.length) * STEP + PAD;

// one end's sockets, centered on the node's middle
function End({ sockets, side }: { sockets: Socket[]; side: "inputs" | "outputs" }) {
    const input = side === "inputs";
    return (
        <>
            {sockets.map((h, i) => (
                <SocketHandle key={h.id} id={h.id} side={side} type={input ? "source" : "target"} position={input ? Position.Left : Position.Right} style={{ width: `${SIZE}px`, height: `${SIZE}px`, top: `calc(50% + ${(i - (sockets.length - 1) / 2) * STEP}px)`, ...socketStyle(h.data_type) }} />
            ))}
        </>
    );
}

// the sockets of a collapsed node, as they'd be drawn expanded
function CollapsedSockets({ inputs, outputs }: { inputs: Socket[]; outputs: Socket[] }) {
    return (
        <>
            <End sockets={inputs} side="inputs" />
            <End sockets={outputs} side="outputs" />
        </>
    );
}

export default CollapsedSockets;
