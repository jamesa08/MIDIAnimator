// @ts-nocheck
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

export default NodeHeader;


