import { useCallback, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/nodeEntries";
import { useSetInputs } from "../utils/graphOps";

function set_channel({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("set_channel");
    const [channel, setChannel] = useState(data.inputs?.channel || "");

    useEffect(() => {
        setChannel(data.inputs?.channel || "");
    }, [data.inputs?.channel]);

    // empty leaves the curve on its own channel
    const handleUpdate = useCallback(() => {
        setInputs(id, { channel });
    }, [id, channel, setInputs]);

    const channelComponent = (
        <div>
            <input type="text" className="node-field border border-gray-400 rounded px-2 py-1" value={channel} onChange={(e) => setChannel(e.target.value)} onBlur={handleUpdate} />
        </div>
    );

    return <BaseNode nodeData={nodeData} inject={{ channel: channelComponent }} hidden={{}} data={data} />;
}

export default set_channel;
