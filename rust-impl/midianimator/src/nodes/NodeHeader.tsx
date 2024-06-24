// @ts-nocheck
import React from "react";
import * as st from "../styles.tsx";

function NodeHeader({ label, type }: {label: any, type: any}) {
    return (
        <div
            className="node-header"
            style={{
                background: st.HEADER_COLORS[type],
                textShadow: st.TEXT_SHADOW,
            }}
        >
            {label}
        </div>
    );
}

NodeHeader.displayName = 'NodeHeader';

export default NodeHeader;


