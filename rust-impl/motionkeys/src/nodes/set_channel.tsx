import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { useSetInputs } from "../utils/graphOps";
import { TextBox } from "../components/TextBox";
import { useConnectedInputs } from "../components/nodegraph/NodeFields";

function set_channel({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("set_channel");
    const connected = useConnectedInputs(id);

    // typed in beside its socket unless something is connected to it. empty leaves the curve on its own channel,
    // emptied unsets it
    const channelComponent = connected.includes("channel") ? undefined : <TextBox label="Channel" value={data.inputs?.channel ?? ""} readOnly={data === "preview"} onCommit={(text) => setInputs(id, { channel: text === "" ? null : text })} />;

    return <BaseNode nodeData={nodeData} inject={{}} labels={{ channel: channelComponent }} hidden={{}} data={data} />;
}

export default set_channel;
