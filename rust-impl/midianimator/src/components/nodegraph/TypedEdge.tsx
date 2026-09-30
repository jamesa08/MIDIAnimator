import { useId, useMemo } from "react";
import { BaseEdge, EdgeProps, getBezierPath, useInternalNode } from "@xyflow/react";
import { useGroupContext } from "../../contexts/GroupContext";
import { inputHandle, outputHandle, specLookup } from "../../utils/groups";
import { useNodeSpecs } from "../../utils/nodeEntries";
import { SOCKET_COLORS } from "../../styles";
import { socketCategory } from "../../utils/sockets";

// an edge in the color of the sockets it connects, fading from one to the other when their types differ
function TypedEdge({ id, source, target, sourceHandleId, targetHandleId, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, selected, style, interactionWidth }: EdgeProps) {
    const { groups, scope } = useGroupContext();
    const specs = useNodeSpecs();
    const lookup = useMemo(() => specLookup(Object.fromEntries(specs.map((spec: any) => [spec.id, spec])), groups), [specs, groups]);
    const sourceNode = useInternalNode(source);
    const targetNode = useInternalNode(target);
    // the same edge id can be drawn twice (an open group's frozen parent), gradient ids have to be unique on the page
    const gradientId = `edge-gradient-${useId()}`;

    // inputs are react flow's source handles and outputs its targets, data flows from the target to the source
    const from = targetNode ? SOCKET_COLORS[socketCategory(outputHandle(lookup, targetNode, targetHandleId ?? "", scope).data_type)] : SOCKET_COLORS.any;
    const to = sourceNode ? SOCKET_COLORS[socketCategory(inputHandle(lookup, sourceNode, sourceHandleId ?? "", scope).data_type)] : SOCKET_COLORS.any;
    const [path] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });

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
                </defs>
            )}
            <BaseEdge id={id} path={path} interactionWidth={interactionWidth} style={{ ...style, stroke: from === to ? from : `url(#${gradientId})`, strokeWidth: selected ? 4 : undefined }} />
        </>
    );
}

export default TypedEdge;
