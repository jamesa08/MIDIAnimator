import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { useSetInputs } from "../utils/graphOps";
import { TextBox } from "../components/TextBox";
import { CheckboxRow, useConnectedInputs } from "../components/nodegraph/NodeFields";

function static_property({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("static_property");
    const connected = useConnectedInputs(id);

    // a custom property's name, or a bridge channel id with Custom Property off, typed in beside its socket unless
    // something is connected to it. emptied unsets it
    const channelComponent = connected.includes("channel") ? undefined : <TextBox label="Channel" value={data.inputs?.channel ?? ""} readOnly={data === "preview"} onCommit={(text) => setInputs(id, { channel: text === "" ? null : text })} />;

    // unset is on, the spec's default
    const customProperty = data.inputs?.custom_property ?? true;
    const rescale = data.inputs?.rescale ?? true;

    const customPropertyComponent = <CheckboxRow label="Custom Property" checked={customProperty} onChange={(checked) => setInputs(id, { custom_property: checked })} />;

    const rescaleComponent = <CheckboxRow label="Rescale" checked={rescale} onChange={(checked) => setInputs(id, { rescale: checked })} />;

    return <BaseNode nodeData={nodeData} inject={{ custom_property: customPropertyComponent, rescale: rescaleComponent }} labels={{ channel: channelComponent }} hidden={{ custom_property: true, rescale: true }} data={data} />;
}

export default static_property;
