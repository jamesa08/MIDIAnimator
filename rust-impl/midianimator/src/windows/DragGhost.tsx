import React, { useEffect, useState } from "react";
import { ReactFlowProvider } from "@xyflow/react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import nodeTypes from "../nodes/NodeTypes";
import { DRAG_GHOST_PAD, DRAG_GHOST_SET_EVENT } from "../utils/panels";

// transparent click through window that shows the node being dragged out of a floating panel.
// every node is rendered up front, their data loads async and would pop in if rendered on demand
const DragGhost: React.FC = () => {
    const [ghost, setGhost] = useState<{ nodeType: string | null; width: number }>({ nodeType: null, width: 108 });

    useEffect(() => {
        // the window is transparent, the page has to be too
        document.documentElement.style.background = "transparent";
        document.body.style.background = "transparent";

        invoke("drag_ghost_init");
        const unlisten = listen(DRAG_GHOST_SET_EVENT, (event: any) => setGhost(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    return (
        <ReactFlowProvider>
            <div style={{ padding: DRAG_GHOST_PAD, opacity: 0.75 }}>
                {Object.entries(nodeTypes).map(([key, Node]: [string, any]) => (
                    <div key={key} style={{ width: ghost.width, display: key === ghost.nodeType ? "block" : "none" }}>
                        <Node data="preview" />
                    </div>
                ))}
            </div>
        </ReactFlowProvider>
    );
};

export default DragGhost;
