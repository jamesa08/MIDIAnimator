// @ts-nocheck
// colored by the --node-header var its node sets (styles.tsx nodeColors)
function NodeHeader({ label, italic, children }: { label: any; italic?: boolean; children?: any }) {
    return (
        <div
            className="node-header"
            style={{
                display: "flex",
                alignItems: "center",
            }}
        >
            <span style={{ flex: 1, fontStyle: italic ? "italic" : undefined }}>{label}</span>
            {children}
        </div>
    );
}

export default NodeHeader;
