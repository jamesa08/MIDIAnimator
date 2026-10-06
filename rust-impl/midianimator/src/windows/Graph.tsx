import { ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useStateContext } from "../contexts/StateContext";
import { useStateSync } from "../utils/graphOps";
import { PATH_SEP, resolvePath, specLookup } from "../utils/groups";
import { useNodeSpecs } from "../utils/nodeEntries";
import { NodeCurves, ShownChannel, colorChannels } from "../utils/curves";
import { usePanelGroups } from "../components/PanelBody";
import CurveCanvas, { CurveCanvasHandle } from "../components/graph/CurveCanvas";
import { useKeymap } from "../utils/keymap";
import { Icon } from "../icons";

// a box that's ticked or not (PropertiesPanel's Check), clicking it toggles
function Check({ checked, onToggle }: { checked: boolean; onToggle: () => void }) {
    return (
        <span
            className="properties-check"
            onClick={(event) => {
                event.stopPropagation();
                onToggle();
            }}
        >
            {checked && (
                <svg width="9" height="7" viewBox="0 0 9 7">
                    <path d="M0.5 3.5l2.5 2.5 5.5-5.5" fill="none" stroke="currentColor" />
                </svg>
            )}
        </span>
    );
}

// on macOS the traffic lights sit on the toolbar's left (src-tauri/src/ui/windows.rs open_window), it starts after them
const TRAFFIC_LIGHTS_WIDTH = navigator.userAgent.includes("Mac") ? 78 : 6;

// a toolbar button, like the main window's (Tool.tsx). `active` for a toggle that's on
function ToolButton({ active, onClick, children }: { active?: boolean; onClick: () => void; children: ReactNode }) {
    return (
        <button className={`toolbar-button flex items-center justify-center h-7 min-w-7 px-1 ${active ? "bg-black text-white" : "hover:bg-zinc-100"}`} onClick={onClick}>
            {children}
        </button>
    );
}

// heroicons' outline icons at the toolbar's size
function ToolIcon({ d }: { d: string }) {
    return (
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-5">
            <path strokeLinecap="round" strokeLinejoin="round" d={d} />
        </svg>
    );
}

// the graph window: a toolbar on top, a list of the selected nodes' channels down the left (tick to show, click a name
// to show only that) and their curves from the last run on the right
function Graph() {
    useStateSync();
    const { backEndState: state } = useStateContext();
    const groups = usePanelGroups();
    const specs = useNodeSpecs();

    // the selected nodes in the open group and their keys in the results, like the properties panel finds them
    const project = state.rf_instance;
    const openGroup: string = state.open_group ?? "";
    const levels = useMemo(() => (project?.nodes ? resolvePath(project, groups, openGroup ? openGroup.split(PATH_SEP) : []) : []), [project, groups, openGroup]);
    const level = levels[levels.length - 1];
    const path = levels
        .slice(1)
        .map((l) => l.nodeId)
        .join(PATH_SEP);
    const selected: any[] = (level?.graph.nodes ?? []).filter((node: any) => node.selected);
    const nodeKey = (node: any) => (path ? `${path}${PATH_SEP}${node.id}` : node.id);
    const keys = selected.map(nodeKey).join("\n");
    const lookup = specLookup(Object.fromEntries(specs.map((spec: any) => [spec.id, spec])), groups);
    const names: Record<string, string> = Object.fromEntries(selected.map((node) => [nodeKey(node), node.data?.label || (lookup(node, level?.def ?? null) as any)?.name || node.type]));

    const tab: string = state.active_tab ?? "";
    const [curves, setCurves] = useState<NodeCurves[]>([]);
    useEffect(() => {
        if (!tab || !keys) {
            setCurves([]);
            return;
        }
        let current = true;
        invoke<NodeCurves[]>("graph_curves", { tab, nodes: keys.split("\n") })
            .then((next) => current && setCurves(next))
            .catch((e) => console.error(`Error getting curves: ${e}`));
        return () => {
            current = false;
        };
    }, [tab, keys, state.executed_results]);

    // times in seconds or frames (the graph.time_units setting), frames at the scene's rate. 24 until Blender has sent it.
    // key handles shown or not (graph.show_handles)
    const [frames, setFrames] = useState(false);
    const [showHandles, setShowHandles] = useState(true);
    useEffect(() => {
        const apply = (settings: any) => {
            setFrames(settings?.graph?.time_units === "frames");
            setShowHandles(settings?.graph?.show_handles !== false);
        };
        invoke("get_settings").then(apply);
        const unlisten = listen("settings_changed", (event: any) => apply(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);
    const setFramesSetting = (on: boolean) => invoke("set_setting", { path: "graph.time_units", value: on ? "frames" : "seconds" }).catch((e) => console.error(`Error saving setting graph.time_units: ${e}`));
    const scene = state.scene_data?.["Scene"] ?? Object.values(state.scene_data ?? {})[0];
    const fps: number = typeof (scene as any)?.fps === "number" && (scene as any).fps > 0 ? (scene as any).fps : 24;
    const toggleHandles = () => invoke("set_setting", { path: "graph.show_handles", value: !showHandles }).catch((e) => console.error(`Error saving setting graph.show_handles: ${e}`));
    useKeymap("graph_editor", { show_seconds: () => setFramesSetting(!frames), show_handles: toggleHandles });

    // the view's own commands, for the toolbar
    const canvas = useRef<CurveCanvasHandle>(null);

    const channels = useMemo(() => colorChannels(curves), [curves]);
    const [hidden, setHidden] = useState<Set<string>>(new Set());
    const shown = useMemo(() => channels.filter((channel) => !hidden.has(channel.key)), [channels, hidden]);

    // shows or hides channels together, hidden if any of them is showing
    const toggle = (some: ShownChannel[]) =>
        setHidden((prev) => {
            const next = new Set(prev);
            const hide = some.some((channel) => !prev.has(channel.key));
            some.forEach((channel) => (hide ? next.add(channel.key) : next.delete(channel.key)));
            return next;
        });
    // shows only these channels, or all of them again if that's already what's showing
    const solo = (some: ShownChannel[]) =>
        setHidden((prev) => {
            const only = new Set(some.map((channel) => channel.key));
            const already = channels.every((channel) => prev.has(channel.key) !== only.has(channel.key));
            return already ? new Set() : new Set(channels.filter((channel) => !only.has(channel.key)).map((channel) => channel.key));
        });

    const rows: JSX.Element[] = [];
    for (const node of curves) {
        const nodeChannels = channels.filter((channel) => channel.key.startsWith(`${node.node}\n`));
        if (nodeChannels.length === 0) continue;
        rows.push(
            <div key={node.node} className="properties-section pr-1.5" onClick={() => solo(nodeChannels)}>
                <Check checked={nodeChannels.some((channel) => !hidden.has(channel.key))} onToggle={() => toggle(nodeChannels)} />
                <span className="truncate">{names[node.node] ?? node.node}</span>
            </div>
        );
        const groupNames = [...new Set(nodeChannels.map((channel) => channel.group))];
        const groupRows: JSX.Element[] = [];
        for (const group of groupNames) {
            const groupChannels = nodeChannels.filter((channel) => channel.group === group);
            if (group) {
                groupRows.push(
                    <div key={`group\n${group}`} className="scene-tree-row" onClick={() => solo(groupChannels)}>
                        <Check checked={groupChannels.some((channel) => !hidden.has(channel.key))} onToggle={() => toggle(groupChannels)} />
                        <Icon name="object" />
                        <span className="scene-tree-label">{group}</span>
                    </div>
                );
            }
            for (const channel of groupChannels) {
                groupRows.push(
                    <div key={channel.key} className="scene-tree-row" style={{ paddingLeft: group ? 19 : 0 }} onClick={() => solo([channel])}>
                        <Check checked={!hidden.has(channel.key)} onToggle={() => toggle([channel])} />
                        <span className="w-2.5 h-2.5 flex-none" style={{ backgroundColor: channel.color }} />
                        <span className="scene-tree-label">{channel.name}</span>
                    </div>
                );
            }
        }
        rows.push(
            <div key={`rows\n${node.node}`} className="properties-rows">
                {groupRows}
            </div>
        );
    }

    return (
        <div data-live-resize="window" data-keymap-area="graph_editor" className="w-screen h-screen flex flex-col overflow-hidden select-none font-[Arial,sans-serif]">
            <div data-tauri-drag-region className="toolbar flex-none h-9 flex items-center gap-0.5 pr-1.5 border-b border-[var(--chrome-line)]" style={{ paddingLeft: TRAFFIC_LIGHTS_WIDTH }}>
                <ToolButton onClick={() => canvas.current?.zoomIn()}>
                    <ToolIcon d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607ZM10.5 7.5v6m3-3h-6" />
                </ToolButton>
                <ToolButton onClick={() => canvas.current?.zoomOut()}>
                    <ToolIcon d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607ZM13.5 10.5h-6" />
                </ToolButton>
                <ToolButton onClick={() => canvas.current?.frameAll()}>
                    <ToolIcon d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15" />
                </ToolButton>
                <div className="spacer h-5 w-[1px] mx-1" />
                {/* a curve through a key with its handles */}
                <ToolButton active={showHandles} onClick={toggleHandles}>
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-5">
                        <path strokeLinecap="round" d="M3 19c3.5 0 4.5-11 9-11s5.5 11 9 11" />
                        <path d="M6.5 8h11" />
                        <circle cx="5" cy="8" r="1.5" />
                        <circle cx="19" cy="8" r="1.5" />
                        <circle cx="12" cy="8" r="1.5" fill="currentColor" />
                    </svg>
                </ToolButton>
                <div className="ml-auto flex border border-black text-xs">
                    <button className={`h-6 px-2 ${frames ? "hover:bg-zinc-100" : "bg-black text-white"}`} onClick={() => setFramesSetting(false)}>
                        Seconds
                    </button>
                    <button className={`h-6 px-2 border-l border-black ${frames ? "bg-black text-white" : "hover:bg-zinc-100"}`} onClick={() => setFramesSetting(true)}>
                        Frames
                    </button>
                </div>
            </div>
            <div className="flex flex-auto min-h-0">
                <div className="properties w-56 flex-none overflow-y-auto border-r border-[var(--properties-line)]">{rows}</div>
                <div className="flex-auto min-w-0">
                    <CurveCanvas ref={canvas} channels={shown} frameKey={keys} fps={fps} frames={frames} showHandles={showHandles} />
                </div>
            </div>
        </div>
    );
}

export default Graph;
