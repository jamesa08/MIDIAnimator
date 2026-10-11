import { useCallback, useId, useMemo } from "react";
import { BaseEdge, EdgeLabelRenderer, EdgeProps, getBezierPath, ReactFlowState, useInternalNode, useStore } from "@xyflow/react";
import { useGroupContext } from "../../contexts/GroupContext";
import { inputHandle, outputHandle, specLookup } from "../../utils/groups";
import { useNodeSpecs } from "../../utils/nodeEntries";
import { SOCKET_COLORS } from "../../styles";
import { multiSlotOffset, socketCategory } from "../../utils/sockets";
import ErrorBadge, { useBadInput } from "./ErrorBadge";

// a lighter shade of a color, what a selection is drawn in (a selected node's ring in index.css)
export const lighter = (color: string) => `color-mix(in srgb, ${color} 50%, white)`;

// a selected edge's ring and glow, like a selected node's: a lighter shade of its own color drawn wider under it.
// `stroke` is a color or a gradient's url, `width` the edge's own stroke width
export function EdgeRing({ path, stroke, width = 3 }: { path: string; stroke: string; width?: number }) {
    return (
        <>
            <path className="edge-ring" d={path} style={{ stroke, strokeWidth: width + 10, opacity: 0.35 }} />
            <path className="edge-ring" d={path} style={{ stroke, strokeWidth: width + 4 }} />
        </>
    );
}

// an edge in the color of the sockets it connects, fading from one to the other when their types differ.
// one whose value was the wrong type for its input in the last run is drawn red with a warning sign on it
function TypedEdge({ id, source, target, sourceHandleId, targetHandleId, sourceX, sourceY: socketY, targetX, targetY, sourcePosition, targetPosition, selected, style, interactionWidth }: EdgeProps) {
    const { groups, scope } = useGroupContext();
    const specs = useNodeSpecs();
    const lookup = useMemo(() => specLookup(Object.fromEntries(specs.map((spec: any) => [spec.id, spec])), groups), [specs, groups]);
    const sourceNode = useInternalNode(source);
    const targetNode = useInternalNode(target);
    // the same edge id can be drawn twice (an open group's frozen parent), gradient ids have to be unique on the page
    const gradientId = `edge-gradient-${useId()}`;
    const bad = useBadInput(source, sourceHandleId);

    // inputs are react flow's source handles and outputs its targets, data flows from the target to the source
    const from = targetNode ? SOCKET_COLORS[socketCategory(outputHandle(lookup, targetNode, targetHandleId ?? "", scope).data_type)] : SOCKET_COLORS.any;
    const input = sourceNode ? inputHandle(lookup, sourceNode, sourceHandleId ?? "", scope) : null;
    const to = input ? SOCKET_COLORS[socketCategory(input.data_type)] : SOCKET_COLORS.any;

    // into a multi input, the edge ends at its own slot on the socket, in the order the links were made
    const slot = useCallback(
        (s: ReactFlowState) => {
            const into = s.edges.filter((e) => e.source === source && e.sourceHandle === sourceHandleId);
            return `${into.findIndex((e) => e.id === id)}/${into.length}`;
        },
        [id, source, sourceHandleId]
    );
    const [index, links] = useStore(slot).split("/").map(Number);
    const sourceY = input?.multi && index >= 0 ? socketY + multiSlotOffset(index, links) : socketY;
    const [path, labelX, labelY] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });

    if (bad) {
        return (
            <>
                {selected && <EdgeRing path={path} stroke={lighter("var(--error-edge)")} width={6} />}
                <BaseEdge id={id} path={path} interactionWidth={interactionWidth} style={{ ...style, stroke: "var(--error-edge)", strokeWidth: 6 }} />
                <EdgeLabelRenderer>
                    <div className="edge-error" style={{ transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)` }}>
                        <ErrorBadge message={bad} size={26} />
                    </div>
                </EdgeLabelRenderer>
            </>
        );
    }

    return (
        <>
            {from !== to && (
                <defs>
                    {/* user space, a bounding box gradient disappears on a perfectly straight edge */}
                    <linearGradient id={gradientId} gradientUnits="userSpaceOnUse" x1={targetX} y1={targetY} x2={sourceX} y2={sourceY}>
                        {/* as styles, a stop-color attribute can't use the css vars some socket colors are */}
                        <stop offset="0%" style={{ stopColor: from }} />
                        <stop offset="100%" style={{ stopColor: to }} />
                    </linearGradient>
                    {selected && (
                        <linearGradient id={`${gradientId}-ring`} gradientUnits="userSpaceOnUse" x1={targetX} y1={targetY} x2={sourceX} y2={sourceY}>
                            <stop offset="0%" style={{ stopColor: lighter(from) }} />
                            <stop offset="100%" style={{ stopColor: lighter(to) }} />
                        </linearGradient>
                    )}
                </defs>
            )}
            {selected && <EdgeRing path={path} stroke={from === to ? lighter(from) : `url(#${gradientId}-ring)`} />}
            <BaseEdge id={id} path={path} interactionWidth={interactionWidth} style={{ ...style, stroke: from === to ? from : `url(#${gradientId})` }} />
        </>
    );
}

export default TypedEdge;
