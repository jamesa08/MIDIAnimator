import { resolveResource } from '@tauri-apps/api/path';
import { BaseDirectory, readTextFile } from '@tauri-apps/plugin-fs';



// window event fired when a node is dragged out of the nodes panel and released.
// detail: { nodeType, clientX, clientY, offsetX, offsetY }, offset is where the node was grabbed in node (unscaled) pixels
export const NODE_DROP_EVENT = "motionkeys:node-drop";

export async function getNodeData(nodeId: string) {
    const specs = await loadNodeSpecs();
    if (specs == null) {
        console.log("error finding data for node ", nodeId);
        return {"id": "", "name": "error", handles: {}};
    }
    return specs.find((node: any) => node["id"] === nodeId);
}

// every node spec in default_nodes.json, read once
let nodeSpecs: Promise<any[]> | null = null;
export function loadNodeSpecs(): Promise<any[]> {
    nodeSpecs ??= readTextFile("src/configs/default_nodes.json", { baseDir: BaseDirectory.Resource }).then((data) => JSON.parse(data)["nodes"]);
    return nodeSpecs;
}
