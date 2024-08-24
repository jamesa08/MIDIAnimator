// @ts-nocheck
import React, { ReactNode, useCallback, useState, useEffect } from "react";
import { Handle, NodeResizeControl, Position } from "@xyflow/react";
import "@xyflow/react/dist/base.css";
import NodeHeader from "./NodeHeader";
import { memo } from "react";
import { useDimensions } from "../hooks/useDimensions";


const handleStyle = {
    width: "16px",
    height: "16px",
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
/// @param data: reactflow data
/// @param children: may be removed later
function BaseNode({ nodeData, inject, hidden, executor, data, children }: { nodeData: any; inject?: any; executor?: any; hidden?: any; data: any; children?: ReactNode }) {
    // iterate over handles
    let handleObjects = [];


    if (nodeData != null) {
        const handleTypes = ["outputs", "inputs"];
        for (let handleType of handleTypes) {
            let rfHandleType: boolean = false;
            if (handleType == "inputs") {
                rfHandleType = true;
            }

            for (let handleIndex in nodeData["handles"][handleType]) {
                let handle = nodeData["handles"][handleType][handleIndex];

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
                            <Handle id={handle["id"]} type={rfHandleType ? "source" : "target"} position={rfHandleType ? Position.Left : Position.Right} style={rfHandleType ? { ...handleStyle, left: "-20px" } : { ...handleStyle, right: "-20px" }} ></Handle>
                        </div>
                        {uiInject}
                    </>
                );
                handleObjects.push(buildHandle);
            }
        }
    }

    let preview = data != undefined && data == "preview" ? true : false;

    return (
        <div className={`node${preview ? " preview" : ""}`} draggable={preview}>
            <NodeHeader label={nodeData == null ? "" : nodeData["name"]} type={"TRANSFORM"} />
            <NodeResizeControl minWidth={200} maxWidth={1000} variant="line"/>
            <div className="node-inner flex flex-col">{handleObjects.map((handle) => handle)}</div>
        </div>
    );
}

export default memo(BaseNode);
