import { useCallback, useEffect, useState } from "react";
import "@xyflow/react/dist/base.css";
import BaseNode from "./BaseNode";
import { useStateContext } from "../contexts/StateContext";
import { getNodeData } from "../utils/node";
import { useReactFlow } from "@xyflow/react";

type StringifyOptions = {
    maxDepth?: number;
    indentSize?: number;
    maxKeysPerObject?: number;
    maxArrayItems?: number;
    maxStringLength?: number;
    compactThreshold?: number;
};

export function customStringify(obj: any, options: StringifyOptions = {}): string {
    const { maxDepth = 2, indentSize = 2, maxKeysPerObject = 20, maxArrayItems = 20, maxStringLength = 300, compactThreshold = 80 } = options;

    const truncate = (str: string) => (str.length > maxStringLength ? str.slice(0, maxStringLength) + "…" : str);

    const isPrimitive = (v: any) => v === null || v === undefined || ["string", "number", "boolean", "function", "symbol"].includes(typeof v);

    const primitive = (val: any): string => {
        if (val === null || typeof val === "undefined") return "";
        if (typeof val === "string") return `"${truncate(val)}"`;
        if (typeof val === "number" || typeof val === "boolean") return String(val);
        if (typeof val === "function") return `[Function ${val.name || "anonymous"}]`;
        if (typeof val === "symbol") return val.toString();
        return "";
    };

    const renderCompact = (value: any, depth: number, seen: Set<any>): string => {
        if (isPrimitive(value)) return primitive(value);
        if (seen.has(value)) return "[Circular]";
        if (depth >= maxDepth) return Array.isArray(value) ? `[Array(${value.length})]` : "[Object]";

        const nextSeen = new Set(seen);
        nextSeen.add(value);

        if (Array.isArray(value)) {
            const items = value.slice(0, maxArrayItems).map((v) => renderCompact(v, depth + 1, nextSeen));
            const trailing = value.length > maxArrayItems ? `, … ${value.length - maxArrayItems} more` : "";
            return `[${items.join(", ")}${trailing}]`;
        }

        const keys = Object.keys(value).sort();
        const limited = keys.slice(0, maxKeysPerObject);
        const entries = limited.map((k) => `${k}: ${renderCompact(value[k], depth + 1, nextSeen)}`);
        const trailing = keys.length > maxKeysPerObject ? `, … ${keys.length - maxKeysPerObject} more` : "";
        return `{ ${entries.join(", ")}${trailing} }`;
    };

    const renderPretty = (value: any, depth: number, seen: Set<any>): string => {
        if (isPrimitive(value)) return primitive(value);
        if (seen.has(value)) return "[Circular]";
        if (depth >= maxDepth) return Array.isArray(value) ? `[Array(${value.length})]` : "[Object]";

        const indent = " ".repeat(depth * indentSize);
        const innerIndent = " ".repeat((depth + 1) * indentSize);

        // Try compact for the whole value first — if it fits, no need to expand
        const compactAttempt = renderCompact(value, depth, new Set(seen));
        if (compactAttempt.length <= compactThreshold) return compactAttempt;

        const nextSeen = new Set(seen);
        nextSeen.add(value);

        if (Array.isArray(value)) {
            const items = value.slice(0, maxArrayItems).map((v) => {
                const compact = renderCompact(v, depth + 1, new Set(nextSeen));
                return compact.length <= compactThreshold ? compact : renderPretty(v, depth + 1, nextSeen);
            });
            const trailing = value.length > maxArrayItems ? `\n${innerIndent}… ${value.length - maxArrayItems} more items` : "";
            return `[\n${items.map((i) => `${innerIndent}${i}`).join(",\n")}${trailing}\n${indent}]`;
        }

        const keys = Object.keys(value).sort();
        const limited = keys.slice(0, maxKeysPerObject);
        const entries = limited.map((k) => {
            const compact = renderCompact(value[k], depth + 1, new Set(nextSeen));
            const rendered = compact.length <= compactThreshold ? compact : renderPretty(value[k], depth + 1, nextSeen);
            return `${innerIndent}${k}: ${rendered}`;
        });
        const trailing = keys.length > maxKeysPerObject ? `\n${innerIndent}… ${keys.length - maxKeysPerObject} more keys` : "";
        return `{\n${entries.join(",\n")}${trailing}\n${indent}}`;
    };

    const compact = renderCompact(obj, 0, new Set());
    if (compact.length <= compactThreshold) return truncate(compact);

    return renderPretty(obj, 0, new Set());
}

function viewer({ id, data, isConnectable }: { id: any; data: any; isConnectable: any }) {
    const { updateNodeData } = useReactFlow();
    const { backEndState: state, setBackEndState: setState } = useStateContext();

    const [nodeData, setNodeData] = useState<any | null>(null);
    const [viewerData, setViewerData] = useState<any | null>(null);
    const [copied, setCopied] = useState(false);

    const handleCopy = () => {
        navigator.clipboard.writeText(viewerData ?? "");
        setCopied(true);
        setTimeout(() => setCopied(false), 1000);
    };

    useEffect(() => {
        var tempViewerData: any = "";
        if (state != undefined && state.executed_results != undefined && id != undefined) {
            if (id in state.executed_inputs && id in state.executed_results) {
                tempViewerData = tempViewerData = customStringify(state.executed_inputs[id]["data"], {
                    maxDepth: 4,
                    compactThreshold: 100,
                    maxKeysPerObject: 4,
                    maxArrayItems: 15,
                });

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
            <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: 4 }}>
                <button onClick={handleCopy} style={{ position: "absolute", top: 30, right: 5, fontSize: 11, padding: "2px 8px", cursor: "pointer", backgroundColor: copied ? "#4caf50" : "#2196f3", color: "white", border: "none", borderRadius: 4 }}>
                    {copied ? "Copied!" : "Copy"}
                </button>
            </div>
            <div className="node-field node-viewer" style={{ fontFamily: "monospace", whiteSpace: "pre-wrap", wordBreak: "break-word", overflowWrap: "anywhere", textOverflow: "ellipsis" }}>
                {viewerData}
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
