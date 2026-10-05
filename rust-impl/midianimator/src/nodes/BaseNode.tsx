// @ts-nocheck
import React, { ReactNode, useCallback, useState, useEffect } from "react";
import { Handle, NodeResizeControl, Position, useNodeId, useStore } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import NodeHeader from "./NodeHeader";
import { socketStyle } from "../utils/sockets";
import { nodeColors } from "../styles";
import { memo } from "react";
import ErrorBadge, { useNodeError } from "../components/nodegraph/ErrorBadge";

const handleStyle = {
    width: "14px",
    height: "14px",
    display: "flex",
    justifyContent: "center",
    alignItems: "center",
    position: "absolute",
};

/// base node for creating nodes
/// @param nodeData: the data for the node (NOT reactflow data)
/// @param inject: map of handles with ui elements to inject into the handle
/// @param hidden: map of handles to hide, good for when you want to hide a handle but want to write data to it (ui element)
/// @param executor: function to execute when the node is executed. only should be used for nodes that use JS execution
/// @param dynamicHandles: map of handles to add to the node. looks exactly like handles found in `default_nodes.json`. good for when you want to add handles to a node that are not in the node data, dynamically as a UI feature
/// @param data: reactflow data
/// @param headerExtra: shown at the right end of the header, e.g. the open button on group nodes
/// @param children: may be removed later
function BaseNode({ nodeData, inject, hidden, executor, dynamicHandles, data, headerExtra, children }: { nodeData: any; inject?: any; executor?: any; hidden?: any; dynamicHandles?: any; data: any; headerExtra?: ReactNode; children?: ReactNode }) {
    // iterate over handles
    let handleObjects = [];

    // previews (nodes panel, drag ghost) get "preview", or an object with preview set when they need data (group nodes)
    let preview = data === "preview" || data?.preview === true;
    const error = useNodeError();

    // the inputs something is connected to, joined so the node only draws again when they change.
    // stored edges are reversed, `source`/`sourceHandle` is the node taking the value and its input
    const nodeId = useNodeId();
    const connectedInputs = useCallback((s) => s.edges.flatMap((e) => (e.source === nodeId ? [e.sourceHandle] : [])).join("\n"), [nodeId]);
    const connected = useStore(connectedInputs).split("\n");

    if (nodeData != null) {
        const handleTypes = ["outputs", "inputs"];
        for (let handleType of handleTypes) {
            let rfHandleType: boolean = false;
            if (handleType == "inputs") {
                rfHandleType = true;
            }

            let dynHandleArray = dynamicHandles == null || dynamicHandles[handleType] == undefined ? [] : dynamicHandles[handleType];

            for (let handle of [...nodeData["handles"][handleType], ...dynHandleArray]) {
                let uiInject = <></>;
                let uiHidden = false;

                if (inject != null && inject[handle["id"]] != null) {
                    uiInject = inject[handle["id"]];
                }

                if (hidden != null && hidden[handle["id"]] != null) {
                    uiHidden = hidden[handle["id"]];
                }

                // a value typed in for an input without a widget (properties panel, MCP) shows under it, unless a
                // connection's value is used instead or it's the input's default
                let value = rfHandleType && !preview && !uiHidden && inject?.[handle["id"]] == null && !connected.includes(handle["id"]) ? data?.inputs?.[handle["id"]] : undefined;
                if (value === handle["default"]) value = undefined;

                const buildHandle = (
                    <>
                        <div className={`node-field field-${handleType}`} style={{ position: "relative", display: uiHidden ? "none" : "inherit" }}>
                            <span style={{ float: rfHandleType ? "left" : "right", marginLeft: rfHandleType ? "" : "auto" }}>{handle["name"]}</span>
                            {/* previews live outside a flow, Handle needs its store so draw a look alike with the same classes */}
                            {preview ? (
                                <div className={`react-flow__handle react-flow__handle-${rfHandleType ? "left" : "right"}`} style={{ ...(rfHandleType ? { ...handleStyle, left: "-13px" } : { ...handleStyle, right: "-13px" }), ...socketStyle(handle["data_type"]) }}></div>
                            ) : (
                                <Handle id={handle["id"]} type={rfHandleType ? "source" : "target"} position={rfHandleType ? Position.Left : Position.Right} style={{ ...(rfHandleType ? { ...handleStyle, left: "-13px" } : { ...handleStyle, right: "-13px" }), ...socketStyle(handle["data_type"]) }}></Handle>
                            )}
                        </div>
                        {value != null && <div className="node-field node-value">{String(value)}</div>}
                        {uiInject}
                    </>
                );
                handleObjects.push(buildHandle);
            }
        }
    }

    return (
        <div className={`node${preview ? " preview" : ""}${error && !preview ? " node-error" : ""}`} style={nodeColors(nodeData?.category)}>
            <NodeHeader label={nodeData == null ? "" : (!preview && data?.label) || nodeData["name"]}>
                {error && !preview && <ErrorBadge message={error} size={16} />}
                {headerExtra}
            </NodeHeader>
            <NodeResizeControl minWidth={200} maxWidth={1000} variant="line" />
            <div className="node-inner flex flex-col">
                {handleObjects.map((handle, index) => (
                    <React.Fragment key={index}>{handle}</React.Fragment>
                ))}
            </div>
        </div>
    );
}

export default memo(BaseNode);
