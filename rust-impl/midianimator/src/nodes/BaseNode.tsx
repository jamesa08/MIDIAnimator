// @ts-nocheck
import React, { ReactNode, useCallback, useState, useEffect } from "react";
import { Handle, NodeResizeControl, Position } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import NodeHeader from "./NodeHeader";
import { socketStyle } from "../utils/sockets";
import { memo } from "react";

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
                        {uiInject}
                    </>
                );
                handleObjects.push(buildHandle);
            }
        }
    }

    return (
        <div className={`node${preview ? " preview" : ""}`}>
            <NodeHeader label={nodeData == null ? "" : nodeData["name"]} type={nodeData?.category}>
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
