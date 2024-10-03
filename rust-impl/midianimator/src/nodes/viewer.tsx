import { useCallback, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";

function customStringify(obj: any, depth = 0, maxDepth = 1, maxLength = 300): any {
    // Function to stringify objects or arrays without pretty-print formatting
    const compactStringify = (value: any) => {
        let str = JSON.stringify(value, (key, val) => {
            if (typeof val === "function") return val.toString();
            if (typeof val === "symbol") return val.toString();
            return val;
        });

        // Truncate if string exceeds maxLength
        if (str != undefined && str != null && str.length > maxLength) {
            str = str.slice(0, maxLength) + "...";
        }
        return str;
    };

    // Check if we are at the top-level and apply pretty-printing
    if (depth < maxDepth) {
        if (typeof obj === "object" && obj !== null) {
            let indent = "  ".repeat(depth);
            let entries = Object.entries(obj).map(([key, value]) => `${indent}${key}: ${customStringify(value, depth + 1, maxDepth, maxLength)}`);
            return `{\n${entries.join(",\n")}\n${indent}}`;
        } else if (Array.isArray(obj)) {
            return `[${obj.map((value) => customStringify(value, depth + 1, maxDepth, maxLength)).join(", ")}]`;
        }
    }

    // Once below the top level, use the compact representation
    return compactStringify(obj);
}

function viewer({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    const [viewerData, setViewerData] = useState<any | null>(null);

    useEffect(() => {
        var tempViewerData: any = "";
        if (state != undefined && state.executed_results != undefined && id != undefined) {
            if (id in state.executed_inputs && id in state.executed_results) {
                tempViewerData = customStringify(state.executed_inputs[id]["data"]);

                if (tempViewerData != viewerData) {
                    setViewerData(tempViewerData);
                }
            } else {
                setViewerData("");
            }
        }
    }, [state.executed_results]);

    useEffect(() => {
        getNodeData("viewer").then(setNodeData);
    }, []);

    const viewerComponent = (
        <>
            <div className="node-field node-viewer" style={{ fontFamily: "monospace", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                {viewerData?.split("\n").map((line: any, _: any) => (
                    <>
                        {line}
                        <br />
                    </>
                ))}
            </div>
        </>
    );

    const uiInject = {
        data: viewerComponent,
    };

    const hiddenHandles = {};

    return <BaseNode nodeData={nodeData} inject={uiInject} hidden={hiddenHandles} data={data} />;
}

export default viewer;
