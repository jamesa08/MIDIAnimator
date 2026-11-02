// @ts-nocheck
import { memo } from "react";
import ErrorBoundary from "../components/ErrorBoundary";

type NodeComponentModule = {
    default: React.ComponentType<any>;
};

const nodeComponents: Record<string, NodeComponentModule> = import.meta.glob("./*.tsx", { eager: true });

// Function to convert file names to node names
function convertFileNameToNodeName(fileName: string) {
    return fileName
        .replace("./", "") // remove relative path
        .replace(".tsx", "") // remove file extension
        .replace(/([A-Z])/g, "_$1") // convert camelcase to snake_case
        .toLowerCase(); // convert to lowercase
}

// a node that throws while drawing shows its error in its place, the graph around it keeps working. it's drawn again
// when its data changes
function guarded(key: string, NodeComponent: React.ComponentType<any>) {
    const Guarded = (props: any) => (
        <ErrorBoundary
            name={key}
            resetKeys={[props.data]}
            fallback={(error) => (
                <div className="node node-error node-crashed">
                    <div className="node-header">
                        <span>{props.data?.label || key}</span>
                    </div>
                    <pre className="node-crashed-message">{error.message}</pre>
                </div>
            )}
        >
            <NodeComponent {...props} />
        </ErrorBoundary>
    );
    Guarded.displayName = `Guarded(${key})`;
    return Guarded;
}

let nodeTypes: any = {};
for (const [filePath, componentModule] of Object.entries(nodeComponents)) {
    const key = convertFileNameToNodeName(filePath); // convert file name to node name
    if (key[0] == "_") {
        continue;
    } // skip files that are not nodes
    // memo so a node only redraws when its own props change, not on every graph update
    nodeTypes[key] = memo(guarded(key, componentModule.default));
}

export default nodeTypes;
