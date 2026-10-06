import { resolveResource } from '@tauri-apps/api/path';
import { BaseDirectory, readTextFile } from '@tauri-apps/plugin-fs';



// window event fired when a node is dragged out of the nodes panel and released.
// detail: { nodeType, clientX, clientY, offsetX, offsetY }, offset is where the node was grabbed in node (unscaled) pixels
export const NODE_DROP_EVENT = "motionkeys:node-drop";

// every node spec in default_nodes.json, read once. `loadedNodeSpecs` is set once read, for reading them synchronously
let nodeSpecs: Promise<any[]> | null = null;
export let loadedNodeSpecs: any[] | null = null;
export function loadNodeSpecs(): Promise<any[]> {
    nodeSpecs ??= readTextFile("src/configs/default_nodes.json", { baseDir: BaseDirectory.Resource }).then((data) => (loadedNodeSpecs = JSON.parse(data)["nodes"]));
    return nodeSpecs;
}
