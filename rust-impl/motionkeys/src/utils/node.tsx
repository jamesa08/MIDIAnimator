import { resolveResource } from '@tauri-apps/api/path';
import { BaseDirectory, readTextFile } from '@tauri-apps/plugin-fs';
import { useContext, useEffect, useState } from "react";
import { StateContext } from "../contexts/StateContext";



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

// the node specs, from the state when this window has it.
// read synchronously so a node is drawn filled in on its first frame
export function useNodeSpecs(): any[] {
    const fromState = useContext(StateContext)?.backEndState?.default_nodes?.nodes;
    const [loaded, setLoaded] = useState<any[] | null>(loadedNodeSpecs);
    useEffect(() => {
        if (!fromState && !loaded) loadNodeSpecs().then(setLoaded);
    }, [fromState, loaded]);
    return fromState ?? loaded ?? [];
}

// one node type's spec, null until the specs are read
export function useNodeSpec(nodeType: string): any | null {
    return useNodeSpecs().find((spec) => spec.id === nodeType) ?? null;
}
