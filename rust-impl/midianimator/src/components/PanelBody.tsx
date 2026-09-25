import React, { useEffect, useRef, useState } from "react";
import nodeTypes from "../nodes/NodeTypes";
import { ReactFlowProvider } from "@xyflow/react";
import { PANELS, startDragGhost } from "../utils/panels";

// where a node from the nodes panel was released, offset is where it was grabbed in node (unscaled) pixels
export interface PanelNodeDrop {
    nodeType: string;
    clientX: number;
    clientY: number;
    screenX: number;
    screenY: number;
    offsetX: number;
    offsetY: number;
}

interface PanelBodyProps {
    id: number;
    onNodeDrop: (drop: PanelNodeDrop) => void;
    // draw the dragged node in the drag ghost window so it can follow the cursor out of this window
    ghostWindow?: boolean;
}

// panel contents, shared by the docked panel and its popped out window
const PanelBody: React.FC<PanelBodyProps> = ({ id, onNodeDrop, ghostWindow = false }) => {
    // drag a preview node out of the panel, the node graph adds it where it's released.
    // pointer events instead of html5 drag and drop, tauri's native drop handling swallows html5 drops
    const startNodeDrag = (event: React.PointerEvent<HTMLDivElement>, nodeType: string) => {
        if (event.button !== 0) return;
        const preview = event.currentTarget.querySelector(".node.preview") as HTMLElement | null;
        if (!preview) return;

        // previews are drawn at half scale, grab offset is kept in real node pixels
        const rect = preview.getBoundingClientRect();
        const scale = rect.width / preview.offsetWidth || 0.5;
        const grabX = event.clientX - rect.left;
        const grabY = event.clientY - rect.top;
        const startX = event.clientX;
        const startY = event.clientY;
        let ghost: HTMLElement | null = null;
        let dragging = false;
        // ghost window version, set up now so the node is ready by the time the drag starts
        const ghostWin = ghostWindow ? startDragGhost(nodeType, rect.width) : null;

        const handleMove = (e: PointerEvent) => {
            // small dead zone so a plain click doesn't start a drag
            if (!dragging && Math.hypot(e.clientX - startX, e.clientY - startY) < 4) return;
            if (!dragging) {
                dragging = true;
                document.body.style.cursor = "grabbing";
                if (!ghostWin) {
                    // clone of the preview that follows the cursor
                    ghost = preview.cloneNode(true) as HTMLElement;
                    Object.assign(ghost.style, { position: "fixed", left: "0", top: "0", width: `${preview.offsetWidth}px`, margin: "0", opacity: "0.75", pointerEvents: "none", zIndex: "2000", transformOrigin: "top left", cursor: "grabbing" });
                    document.body.appendChild(ghost);
                }
            }
            if (ghostWin) ghostWin.move(e.screenX - grabX, e.screenY - grabY);
            if (ghost) ghost.style.transform = `translate(${e.clientX - grabX}px, ${e.clientY - grabY}px) scale(${scale})`;
        };

        const cleanup = () => {
            ghostWin?.end();
            ghost?.remove();
            document.body.style.cursor = "";
            window.removeEventListener("pointermove", handleMove);
            window.removeEventListener("pointerup", handleUp);
            window.removeEventListener("keydown", handleKey, true);
        };

        const handleUp = (e: PointerEvent) => {
            cleanup();
            if (!dragging) return;
            onNodeDrop({ nodeType, clientX: e.clientX, clientY: e.clientY, screenX: e.screenX, screenY: e.screenY, offsetX: grabX / scale, offsetY: grabY / scale });
        };

        // escape cancels the drag
        const handleKey = (e: KeyboardEvent) => {
            if (e.key !== "Escape") return;
            e.stopPropagation();
            cleanup();
        };

        event.preventDefault();
        window.addEventListener("pointermove", handleMove);
        window.addEventListener("pointerup", handleUp);
        window.addEventListener("keydown", handleKey, true);
    };

    const ScaledNodeWrapper: React.FC<{ Node: any; nodeType: string }> = ({ Node, nodeType }) => {
        const nodeRef = useRef<HTMLDivElement>(null);
        const [isMeasured, setIsMeasured] = useState(false);

        useEffect(() => {
            if (!nodeRef.current || isMeasured) return;

            const node = nodeRef.current.querySelector(".node.preview") as HTMLElement;
            if (!node) return;

            const observer = new MutationObserver(() => {
                const height = node.scrollHeight;

                if (height > 50) {
                    node.style.marginBottom = `-${height * 0.5}px`;
                    setIsMeasured(true);
                    observer.disconnect();
                }
            });

            observer.observe(node, {
                childList: true,
                subtree: true,
            });

            return () => observer.disconnect();
        }, [isMeasured]);

        return (
            <div ref={nodeRef} className="node-container" onPointerDown={(e) => startNodeDrag(e, nodeType)}>
                <Node data="preview" />
            </div>
        );
    };

    if (PANELS[id]?.name !== "Nodes") return null;

    return (
        <ReactFlowProvider>
            <div className="nodes-grid p-2">
                {Object.entries(nodeTypes).map(([key, value]) => (
                    <ScaledNodeWrapper key={key} Node={value} nodeType={key} />
                ))}
            </div>
        </ReactFlowProvider>
    );
};

export default PanelBody;
