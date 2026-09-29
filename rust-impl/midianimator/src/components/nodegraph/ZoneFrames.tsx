import { useLayoutEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useEdges, useNodes, useStore } from "@xyflow/react";
import { findZones } from "../../utils/groups";

// the frame starts a little inside the zone input and ends a little inside the zone output, so their outer edges
// stick out of it. nodes inside the zone get PAD around them, and there's PAD_Y above and below everything
const INSET = 15;
const PAD = 24;
const PAD_Y = 14;

// draws a frame around each for each zone, under the edges and nodes (React Flow's viewport portal is above them)
function ZoneFrames() {
    const nodes = useNodes();
    const edges = useEdges();
    // React Flow sets its dom node after its children first render
    const domNode = useStore((s) => s.domNode);
    const [container, setContainer] = useState<HTMLElement | null>(null);

    // a layer at the start of the viewport, it moves and zooms with the graph
    useLayoutEffect(() => {
        const viewport = domNode?.querySelector(".react-flow__viewport");
        if (!viewport) return;
        const layer = document.createElement("div");
        layer.className = "zone-frames";
        viewport.prepend(layer);
        setContainer(layer);
        return () => layer.remove();
    }, [domNode]);

    const frames = useMemo(() => {
        const byId = new Map(nodes.map((n) => [n.id, n]));
        return findZones({ nodes, edges })
            .map((zone) => {
                const input = byId.get(zone.input);
                const output = byId.get(zone.output);
                if (!input || !output) return null;
                const body = [...zone.body].map((id) => byId.get(id)).filter((n): n is NonNullable<typeof n> => !!n);
                const members = [input, output, ...body];
                const right = (n: (typeof members)[number]) => n.position.x + (n.measured?.width ?? 200);
                const left = Math.min(input.position.x + INSET, ...body.map((n) => n.position.x - PAD));
                const outerRight = Math.max(right(output) - INSET, ...body.map((n) => right(n) + PAD));
                const top = Math.min(...members.map((n) => n.position.y)) - PAD_Y;
                const bottom = Math.max(...members.map((n) => n.position.y + (n.measured?.height ?? 80))) + PAD_Y;
                const width = outerRight - left;
                const height = bottom - top;
                return { key: zone.input, left, top, width, height };
            })
            .filter((f): f is NonNullable<typeof f> => !!f);
    }, [nodes, edges]);

    if (!container) return null;
    return createPortal(
        frames.map((f) => <div key={f.key} className="zone-frame" style={{ transform: `translate(${f.left}px, ${f.top}px)`, width: f.width, height: f.height }} />),
        container
    );
}

export default ZoneFrames;
