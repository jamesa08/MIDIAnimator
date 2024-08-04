import { resolveResource } from '@tauri-apps/api/path';
import { BaseDirectory, readTextFile } from '@tauri-apps/api/fs';



export async function getNodeData(nodeId: string) {
    let data: any = await readTextFile("src/configs/default_nodes.json", { dir: BaseDirectory.Resource });
    if (data == null) {
        console.log("error finding data for node ", nodeId);
        return {"id": "", "name": "error", handles: {}};
    }
    data = JSON.parse(data);
    return data["nodes"].find((node: any) => node["id"] === nodeId);
}