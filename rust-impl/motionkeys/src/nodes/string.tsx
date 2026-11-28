import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useNodeSpec } from "../utils/node";
import { useSetInputs } from "../utils/graphOps";
import { TextBox } from "../components/TextBox";

function string({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const setInputs = useSetInputs();
    const nodeData = useNodeSpec("string");

    // emptied unsets it
    const textComponent = <TextBox className="node-field" label="String" value={data.inputs?.string ?? ""} readOnly={data === "preview"} onCommit={(text) => setInputs(id, { string: text === "" ? null : text })} />;

    return <BaseNode nodeData={nodeData} inject={{ string: textComponent }} hidden={{ string: true }} data={data} />;
}

export default string;
