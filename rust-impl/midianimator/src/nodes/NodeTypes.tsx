// @ts-nocheck
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

let nodeTypes: any = {};
for (const [filePath, componentModule] of Object.entries(nodeComponents)) {
    const key = convertFileNameToNodeName(filePath); // convert file name to node name
    if (key[0] == "_") {
        continue;
    } // skip files that are not nodes
    nodeTypes[key] = componentModule.default; // get default export from module
}

export default nodeTypes;
