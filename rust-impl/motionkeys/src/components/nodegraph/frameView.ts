import { useCallback } from "react";
import { Node, getViewportForBounds, useReactFlow, useStoreApi } from "@xyflow/react";
import { uncoveredRect } from "../../utils/panels";

// frames `nodes` (every node when there are none) like fitView, but in the part of the canvas the docked panels don't
// cover, so nothing ends up under them
export function useFrameView() {
    const { getNodes, getNodesBounds, setViewport } = useReactFlow();
    const store = useStoreApi();
    return useCallback(
        (nodes?: Node[], { maxZoom = 1, padding = 0.1 } = {}) => {
            const { domNode, minZoom } = store.getState();
            const framed = nodes?.length ? nodes : getNodes();
            if (!domNode || framed.length === 0) return;
            const canvas = domNode.getBoundingClientRect();
            const visible = uncoveredRect(domNode);
            const { x, y, zoom } = getViewportForBounds(getNodesBounds(framed), visible.width, visible.height, minZoom, maxZoom, padding);
            setViewport({ x: x + visible.left - canvas.left, y: y + visible.top - canvas.top, zoom });
        },
        [getNodes, getNodesBounds, setViewport, store]
    );
}
