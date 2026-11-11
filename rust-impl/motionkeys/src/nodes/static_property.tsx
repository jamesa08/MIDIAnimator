import { useCallback, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";

function static_property({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("static_property");
    const [channel, setChannel] = useState(data.inputs?.channel || "");

    useEffect(() => {
        setChannel(data.inputs?.channel || "");
    }, [data.inputs?.channel]);

    // a custom property's name, or a bridge channel id with Custom Property off. only a change is sent, emptied unsets it
    const handleUpdate = useCallback(() => {
        if (channel !== (data.inputs?.channel || "")) setInputs(id, { channel: channel === "" ? null : channel });
    }, [id, channel, data.inputs?.channel, setInputs]);

    const channelComponent = (
        <div>
            <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" value={channel} onChange={(e) => setChannel(e.target.value)} onBlur={handleUpdate} />
        </div>
    );

    // unset is on, the spec's default
    const customProperty = data.inputs?.custom_property ?? true;
    const rescale = data.inputs?.rescale ?? true;

    const customPropertyComponent = (
        <label className="node-field flex items-center gap-1">
            <input type="checkbox" checked={customProperty} onChange={(e) => setInputs(id, { custom_property: e.target.checked })} />
            Custom Property
        </label>
    );

    const rescaleComponent = (
        <label className="node-field flex items-center gap-1">
            <input type="checkbox" checked={rescale} onChange={(e) => setInputs(id, { rescale: e.target.checked })} />
            Rescale
        </label>
    );

    return <BaseNode nodeData={nodeData} inject={{ channel: channelComponent, custom_property: customPropertyComponent, rescale: rescaleComponent }} hidden={{ custom_property: true, rescale: true }} data={data} />;
}

export default static_property;
