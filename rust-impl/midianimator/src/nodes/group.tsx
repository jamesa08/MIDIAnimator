import { useEffect, useMemo } from "react";
import { useUpdateNodeInternals } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useGroupContext } from "../contexts/GroupContext";

// runs a node group, its sockets are the group's interface. Tab (or the button in the header) opens it
function group({ id, data }: { id: any; data: any }) {
    const { groups, openGroup } = useGroupContext();
    const updateNodeInternals = useUpdateNodeInternals();
    const preview = data === "preview" || data?.preview === true;
    const def = groups[data?.group_id];

    const nodeData = useMemo(() => {
        if (!def) return { name: `Missing group '${data?.group_id}'`, category: "group", handles: { inputs: [], outputs: [] } };
        return { name: def.name, category: def.category ? `${def.category}_group` : "group", handles: { inputs: def.interface.inputs, outputs: def.interface.outputs } };
    }, [def, data?.group_id]);

    // sockets change when the group's interface is edited, have React Flow measure them again so edges can be drawn
    const socketsKey = [...nodeData.handles.inputs, ...nodeData.handles.outputs].map((h: any) => h.id).join(",");
    useEffect(() => {
        if (!preview) updateNodeInternals(id);
    }, [id, socketsKey, preview, updateNodeInternals]);

    const openButton = preview ? null : (
        <button
            className="group-open nodrag nopan"
            onClick={(event) => {
                event.stopPropagation();
                openGroup(id);
            }}
        >
            ⧉
        </button>
    );

    return <BaseNode nodeData={nodeData} data={data} headerExtra={openButton} />;
}

export default group;
