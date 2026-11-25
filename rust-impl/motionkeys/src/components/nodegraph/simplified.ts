import { createContext, useContext } from "react";
import { ArrangeMode, ArrangeSocket, arrange } from "../../utils/arrange";
import { isFrame } from "../../utils/frames";
import { collapsedHeight, socketOffset } from "./CollapsedSockets";

// the simplified view (the simplified_view command): every node drawn as a tile, its name over a picture of its result,
// laid out on its own so the whole graph reads at a glance. only how the graph is shown changes, nothing is stored and
// the graph's own positions stay as they are

// whether the graph on screen is shown simplified, given by its editor
export const SimplifiedContext = createContext(false);
export const useSimplified = () => useContext(SimplifiedContext);

// a tile's width, its header's height and its picture's. a node with nothing to picture is its header alone
export const TILE_WIDTH = 220;
export const TILE_HEADER = 28;
export const TILE_PICTURE = 96;
// the room around the picture
const TILE_PAD = 6;
export const TILE_HEIGHT = TILE_HEADER + TILE_PICTURE + 2 * TILE_PAD;

// how long the nodes and the view take to glide when the simplified view comes or goes, in milliseconds
export const SWITCH_MS = 250;

// the input whose value names what a node holds, its title on a tile: a track, an object, a generator, an object group
const TITLE_INPUTS: Record<string, string> = {
    get_midi_track_data: "track_name",
    keyframes_from_object: "object_name",
    animation_generator: "name",
    preset_animation: "mode",
    assign_notes_to_objects: "object_group_name",
};

// a tile's title from the node's typed in values: what it holds, unless the input is connected or not set
export function tileTitle(type: string | undefined, inputs: any, connected: string[]): string | undefined {
    const input = type ? TITLE_INPUTS[type] : undefined;
    const value = input && !connected.includes(input) ? inputs?.[input] : undefined;
    if (typeof value !== "string" || !value.trim()) return undefined;
    // an object group's kind, "collection:Drums"
    return value.replace(/^[a-z_]+:/, "");
}

// the nodes that always picture their result on a tile
const PICTURED = new Set(["get_midi_track_data", "keyframes_from_object", "animation_generator", "preset_animation", "assign_notes_to_objects"]);

// whether a value is keyframes by object (what Evaluate Instrument and Combine Keyframes give)
const isKeyframeMap = (value: any) => !!value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length > 0 && Object.values(value).every((keys: any) => Array.isArray(keys) && (keys.length === 0 || (keys[0]?.key !== undefined && keys[0]?.channel !== undefined)));

// a node's results that are keyframes by object, for its picture of a curve per object
export const keyframesByObject = (results: any): any => (results && typeof results === "object" ? Object.values(results).find(isKeyframeMap) : undefined);

// whether a node's tile has a picture: the nodes that always do, and any whose results are keyframes by object
export const hasPicture = (type: string | undefined, results: any) => (!!type && PICTURED.has(type)) || !!keyframesByObject(results);

type XY = { x: number; y: number };

// where every node goes in the simplified view: arranged like Arrange does (the same mode), as tiles. frames aren't
// shown, so nothing is in one. `pictured` says which nodes' tiles have a picture
export async function tileLayout(nodes: any[], edges: any[], pictured: (node: any) => boolean, mode?: ArrangeMode): Promise<Record<string, XY>> {
    const shown = nodes.filter((n) => !isFrame(n) && !n.hidden);
    const present = new Set(shown.map((n) => n.id));
    const links = edges.filter((e) => present.has(e.source) && present.has(e.target));
    // stored edges are reversed: `source` takes the value into `sourceHandle`, `target` gives it from `targetHandle`
    const connected = (id: string, side: "inputs" | "outputs") => [...new Set(links.flatMap((e) => (side === "inputs" ? (e.source === id ? [e.sourceHandle] : []) : e.target === id ? [e.targetHandle] : [])))];
    const sockets: Record<string, ArrangeSocket[]> = {};
    const sized = shown.map((n) => {
        const [ins, outs] = [connected(n.id, "inputs"), connected(n.id, "outputs")];
        const height = pictured(n) ? TILE_HEIGHT : Math.max(TILE_HEADER, collapsedHeight(ins as any, outs as any));
        sockets[n.id] = [...ins.map((socket, i) => ({ side: "inputs" as const, socket, y: height / 2 + socketOffset(i, ins.length) })), ...outs.map((socket, i) => ({ side: "outputs" as const, socket, y: height / 2 + socketOffset(i, outs.length) }))];
        return { ...n, frame: undefined, width: undefined, height: undefined, measured: { width: TILE_WIDTH, height } };
    });
    return arrange(sized, links, [], sockets, mode);
}
