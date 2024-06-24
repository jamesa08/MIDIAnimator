// @ts-nocheck
import React, { ReactNode, useCallback, useState } from "react";
import { Handle, Position } from "reactflow";
import "reactflow/dist/base.css";

const handleStyle = {
    width: "16px",
    height: "16px",
    display: "flex",
    justifyContent: "center",
    alignItems: "center",
    position: "absolute",
};

function BaseNode({ handles, data, children }: { handles: any; data: any; children: ReactNode }) {
    // iterate over handles
    let handleObjects = [];

    for (const [handleType, listOfHandles] of Object.entries(handles)) {
        for (const [i, handle] of Object.entries(listOfHandles)) {
            let rfHandleType: boolean = false;
            if (handleType == "input") {
                rfHandleType = true;
            }
            let buildHandle = (
                <>
                    <div className={`node-field field-${handleType}`} style={{ position: "relative", display: handle.show ? "inherit" : "none" }}>
                        <span style={{ float: rfHandleType ? "left" : "right", marginLeft: rfHandleType ? "" : "auto"}}>{handle.name}</span>
                        <Handle type={rfHandleType ? "source" : "target"} position={rfHandleType ? Position.Left : Position.Right} style={rfHandleType ? {... handleStyle, left: "-20px"} : {... handleStyle, right: "-20px"}} isConnectable={true}></Handle>
                    </div>
                    {handle.component}
                </>
            );
            handleObjects.push(buildHandle);
        }
    }

    let nodeHeader: ReactNode = null;
    let nodeBody: ReactNode = null;

    React.Children.forEach(children, (child) => {
        if (React.isValidElement(child)) {
            const childType = child.type as React.ComponentType<any>;
            if (childType.displayName === "NodeHeader") {
                nodeHeader = child;
            } else if (childType.displayName === "NodeBody") {
                nodeBody = child;
            }
        }
    });

    let preview = data != undefined && data == "preview" ? true : false;

    return (
        <div className={`node${preview ? " preview" : ""}`} draggable={preview}>
            {nodeHeader}
            <div className="node-inner flex flex-col">
                {handleObjects.map((handle) => handle)}
                {nodeBody}
            </div>
        </div>
    );
}

export default BaseNode;
