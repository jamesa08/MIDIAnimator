import React, { useEffect, useMemo, useState } from "react";
import { ReactFlowProvider } from "@xyflow/react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import nodeTypes from "../nodes/NodeTypes";
import { DRAG_GHOST_PAD, DRAG_GHOST_SET_EVENT } from "../utils/panels";
import { GroupContext } from "../contexts/GroupContext";
import { usePanelGroups } from "../components/PanelBody";
import { nodeEntries, previewData, useNodeSpecs } from "../utils/nodeEntries";

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

    const groups = usePanelGroups();
    const specs = useNodeSpecs();
    const entries = useMemo(() => nodeEntries(groups, specs), [groups, specs]);
    const groupContext = useMemo(() => ({ groups, scope: null, scopeId: null, editable: false, openGroup: () => {} }), [groups]);

    return (
        <ReactFlowProvider>
            <GroupContext.Provider value={groupContext}>
                <div style={{ padding: DRAG_GHOST_PAD, opacity: 0.75 }}>
                    {entries.map((entry) => {
                        const Node = (nodeTypes as any)[entry.nodeType];
                        return (
                            <div key={entry.key} style={{ width: ghost.width, display: entry.key === ghost.nodeType ? "block" : "none" }}>
                                <Node data={previewData(entry)} />
                            </div>
                        );
                    })}
                </div>
            </GroupContext.Provider>
        </ReactFlowProvider>
    );
};

export default DragGhost;
