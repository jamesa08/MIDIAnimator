// @ts-nocheck
import * as st from "../styles.tsx";

function NodeHeader({ label, type, children }: { label: any; type: any; children?: any }) {
    return (
        <div
            className="node-header"
            style={{
                background: st.HEADER_COLORS[type],
                textShadow: st.TEXT_SHADOW,
                display: "flex",
                alignItems: "center",
            }}
        >
            <span style={{ flex: 1 }}>{label}</span>
            {children}
        </div>
    );
}

export default NodeHeader;
