import { ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useStateContext } from "../contexts/StateContext";
import { useStateSync } from "../utils/graphOps";
import { PATH_SEP, resolvePath, specLookup } from "../utils/groups";
import { useNodeSpecs } from "../utils/nodeEntries";
import { NodeCurves, ShownChannel, colorChannels, curveBounds } from "../utils/curves";
import { usePanelGroups } from "../components/PanelBody";
import CurveCanvas, { CurveCanvasHandle } from "../components/graph/CurveCanvas";
import ScenePreview, { SHAPES } from "../components/graph/ScenePreview";
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

// on macOS the traffic lights sit on the toolbar's left (src-tauri/src/ui/windows.rs open_window), it starts after them.
// they don't grow with the page zoom (index.css --zoom)
const TRAFFIC_LIGHTS_WIDTH = navigator.userAgent.includes("Mac") ? "calc(78px / var(--zoom))" : 6;

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
    // key handles shown or not (graph.show_handles), the 3d preview shown or not (graph.show_preview), its shape
    // (graph.preview_shape) and its floor grid shown or not (graph.preview_grid)
    const [frames, setFrames] = useState(false);
    const [showHandles, setShowHandles] = useState(true);
    const [showPreview, setShowPreview] = useState(true);
    const [previewShape, setPreviewShape] = useState(SHAPES[0].id);
    const [showGrid, setShowGrid] = useState(true);
    useEffect(() => {
        const apply = (settings: any) => {
            setFrames(settings?.graph?.time_units === "frames");
            setShowHandles(settings?.graph?.show_handles !== false);
            setShowPreview(settings?.graph?.show_preview !== false);
            setPreviewShape(settings?.graph?.preview_shape ?? SHAPES[0].id);
            setShowGrid(settings?.graph?.preview_grid !== false);
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
    const togglePreview = () => invoke("set_setting", { path: "graph.show_preview", value: !showPreview }).catch((e) => console.error(`Error saving setting graph.show_preview: ${e}`));
    const toggleGrid = () => invoke("set_setting", { path: "graph.preview_grid", value: !showGrid }).catch((e) => console.error(`Error saving setting graph.preview_grid: ${e}`));
    const setShapeSetting = (shape: string) => invoke("set_setting", { path: "graph.preview_shape", value: shape }).catch((e) => console.error(`Error saving setting graph.preview_shape: ${e}`));

    // the view's own commands, for the toolbar
    const canvas = useRef<CurveCanvasHandle>(null);

    const channels = useMemo(() => colorChannels(curves), [curves]);
    const [hidden, setHidden] = useState<Set<string>>(new Set());
    const shown = useMemo(() => channels.filter((channel) => !hidden.has(channel.key)), [channels, hidden]);

    // MARK: Playback

    // the playhead in seconds, always on a whole frame like Blender's current frame
    const [time, setTime] = useState(0);
    const [playing, setPlaying] = useState(false);
    const timeRef = useRef(time);
    timeRef.current = time;
    const toFrame = useCallback((t: number) => Math.round(t * fps) / fps, [fps]);
    // it plays from the first key to the last one of every curve and starts over, on and on with no curves
    const bounds = useMemo(() => curveBounds(channels), [channels]);
    const loopStart = bounds ? toFrame(bounds.x0) : 0;
    const loopEnd = bounds ? toFrame(bounds.x1) : Infinity;

    // playback counts from the playhead's time at a moment, moving the playhead while it plays carries on from there
    const origin = useRef({ wall: 0, time: 0 });
    const seek = useCallback(
        (t: number) => {
            const snapped = toFrame(t);
            origin.current = { wall: performance.now(), time: snapped };
            setTime(snapped);
        },
        [toFrame]
    );
    // while scrubbing it stays where the mouse has it, playing or not, and plays on from there when it's let go (Blender)
    const scrubbing = useRef(false);
    const scrub = useCallback(
        (t: number) => {
            scrubbing.current = true;
            seek(t);
        },
        [seek]
    );
    const endScrub = useCallback(() => {
        scrubbing.current = false;
    }, []);
    const stepFrames = (frames: number) => seek(timeRef.current + frames / fps);
    const jumpToStart = () => seek(loopStart);
    // nowhere to jump with no curves
    const jumpToEnd = () => {
        if (Number.isFinite(loopEnd)) seek(loopEnd);
    };

    // plays in real time from where the playhead is, it moves a frame at a time
    useEffect(() => {
        if (!playing) return;
        origin.current = { wall: performance.now(), time: timeRef.current < loopStart || timeRef.current >= loopEnd ? loopStart : timeRef.current };
        let frame = 0;
        const tick = (now: number) => {
            frame = requestAnimationFrame(tick);
            // held, its clock starts again from where it's held
            if (scrubbing.current) {
                origin.current = { wall: now, time: origin.current.time };
                return;
            }
            // a frame's time can be from just before the playhead was moved
            let t = origin.current.time + Math.max(0, now - origin.current.wall) / 1000;
            if (t > loopEnd) {
                origin.current = { wall: now, time: loopStart };
                t = loopStart;
            }
            setTime(Math.floor(t * fps + 1e-6) / fps);
        };
        frame = requestAnimationFrame(tick);
        return () => cancelAnimationFrame(frame);
    }, [playing, fps, loopStart, loopEnd]);

    useKeymap("graph_editor", {
        show_seconds: () => setFramesSetting(!frames),
        show_handles: toggleHandles,
        play: () => setPlaying((on) => !on),
        previous_frame: () => stepFrames(-1),
        next_frame: () => stepFrames(1),
        jump_to_start: jumpToStart,
        jump_to_end: jumpToEnd,
    });

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
                {/* a cube */}
                <ToolButton active={showPreview} onClick={togglePreview}>
                    <ToolIcon d="m21 7.5-9-5.25L3 7.5m18 0-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9" />
                </ToolButton>
                <div className="spacer h-5 w-[1px] mx-1" />
                {/* jump to start, previous frame, play, next frame, jump to end */}
                <ToolButton onClick={jumpToStart}>
                    <ToolIcon d="M6 6v12M18 6l-9 6 9 6Z" />
                </ToolButton>
                <ToolButton onClick={() => stepFrames(-1)}>
                    <ToolIcon d="M15.75 19.5 8.25 12l7.5-7.5" />
                </ToolButton>
                <ToolButton active={playing} onClick={() => setPlaying((on) => !on)}>
                    {playing ? <ToolIcon d="M5.25 7.5A2.25 2.25 0 0 1 7.5 5.25h9a2.25 2.25 0 0 1 2.25 2.25v9a2.25 2.25 0 0 1-2.25 2.25h-9a2.25 2.25 0 0 1-2.25-2.25v-9Z" /> : <ToolIcon d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.347a1.125 1.125 0 0 1 0 1.972l-11.54 6.347a1.125 1.125 0 0 1-1.667-.986V5.653Z" />}
                </ToolButton>
                <ToolButton onClick={() => stepFrames(1)}>
                    <ToolIcon d="m8.25 4.5 7.5 7.5-7.5 7.5" />
                </ToolButton>
                <ToolButton onClick={jumpToEnd}>
                    <ToolIcon d="M18 6v12M6 6l9 6-9 6Z" />
                </ToolButton>
                <span className="ml-auto mr-2 text-xs">{`${Number(fps.toFixed(3))} fps`}</span>
                <div className="flex border border-black text-xs">
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
                    <CurveCanvas ref={canvas} channels={shown} frameKey={keys} fps={fps} frames={frames} showHandles={showHandles} time={time} onScrub={scrub} onScrubEnd={endScrub} />
                </div>
                {showPreview && (
                    <div className="relative w-80 flex-none border-l border-[var(--properties-line)]">
                        <ScenePreview channels={shown} time={time} shape={previewShape} showGrid={showGrid} />
                        {/* the viewport's own controls over its top left: the shape and the floor grid */}
                        <div className="absolute top-1.5 left-1.5 flex gap-1 text-xs">
                            <select className="border border-black bg-white h-6" value={previewShape} onChange={(e) => setShapeSetting(e.target.value)}>
                                {SHAPES.map((shape) => (
                                    <option key={shape.id} value={shape.id}>
                                        {shape.name}
                                    </option>
                                ))}
                            </select>
                            <button className={`h-6 w-6 flex items-center justify-center border border-black ${showGrid ? "bg-black text-white" : "bg-white hover:bg-zinc-100"}`} onClick={toggleGrid}>
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-4">
                                    <path d="M3.75 3.75h16.5v16.5H3.75zM3.75 9.25h16.5M3.75 14.75h16.5M9.25 3.75v16.5M14.75 3.75v16.5" />
                                </svg>
                            </button>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}

export default Graph;
