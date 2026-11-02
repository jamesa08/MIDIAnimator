// @ts-nocheck
// colored by the --node-header var its node sets (styles.tsx nodeColors). `onToggle` adds the caret that collapses the
// node to this header or expands it, it never drags or selects the node
function NodeHeader({ label, italic, collapsed, onToggle, children }: { label: any; italic?: boolean; collapsed?: boolean; onToggle?: () => void; children?: any }) {
    return (
        <div
            className="node-header"
            style={{
                display: "flex",
                alignItems: "center",
            }}
        >
            {onToggle && (
                <button
                    className="node-caret nodrag nopan"
                    onMouseDown={(e) => e.stopPropagation()}
                    onDoubleClick={(e) => e.stopPropagation()}
                    onClick={(e) => {
                        e.stopPropagation();
                        onToggle();
                    }}
                >
                    <svg width="8" height="8" viewBox="0 0 10 10" style={{ transform: collapsed ? "rotate(-90deg)" : undefined }}>
                        <path d="M1 3l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.5" />
                    </svg>
                </button>
            )}
            <span className="node-label" style={{ flex: 1, fontStyle: italic ? "italic" : undefined }}>
                {label}
            </span>
            {children}
        </div>
    );
}

export default NodeHeader;
