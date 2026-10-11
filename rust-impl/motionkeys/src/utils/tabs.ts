import { invoke } from "@tauri-apps/api/core";
import { useCallback, useRef } from "react";
import { useStateContext } from "../contexts/StateContext";
import { PANELS, panelWindowFrame } from "./panels";
import { takeState } from "./graphOps";
import { showStatus } from "./status";

// the file name at the end of a path
const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

// the floating panels a layout has open
export const floatingPanels = (layout: any) =>
    Object.keys(PANELS)
        .map(Number)
        .filter((id) => layout.panelsShown.includes(id) && layout.panelsPoppedOut.includes(id));

// a layout with where its floating panels' windows are now, they're moved by hand so the layout doesn't follow them
export async function withFloatingFrames(layout: any) {
    const floating = { ...(layout.floating ?? {}) };
    for (const id of floatingPanels(layout)) {
        const frame = await panelWindowFrame(id);
        if (frame) floating[id] = frame;
    }
    return { ...layout, floating };
}

// saves a tab (the one on screen if none is given) to its file, see save_project in src-tauri/src/state/mod.rs. the tab
// on screen's floating windows are where they are now. resolves to the path, rejects with "Save cancelled" if the save
// dialog was cancelled
export function useSaveTab() {
    const { backEndState, frontEndState, setTabLayout } = useStateContext();
    const latest = useRef({ tab: backEndState.active_tab, layout: frontEndState });
    latest.current = { tab: backEndState.active_tab, layout: frontEndState };

    return useCallback(
        async (tab?: string, saveAs = false) => {
            const { tab: shown, layout } = latest.current;
            if (!tab || tab === shown) {
                const placed = await withFloatingFrames(layout);
                setTabLayout(shown, placed);
                await invoke("set_layout", { tab: shown, layout: placed });
            }
            const path = await invoke<string>("save_project", { tab: tab ?? null, saveAs });
            showStatus(`Saved "${fileName(path)}"`);
            return path;
        },
        [setTabLayout]
    );
}

// asks for a project file and opens it in a tab of its own, see load_project in src-tauri/src/state/mod.rs
export function useOpenFile() {
    const { setBackEndState } = useStateContext();
    return useCallback(async () => {
        try {
            const state = await invoke<any>("load_project");
            setBackEndState((s: any) => takeState(s, state));
            const path = state?.tabs?.find((t: any) => t.id === state.active_tab)?.path;
            if (path) showStatus(`Opened "${fileName(path)}"`);
        } catch (error) {
            if (error !== "Load cancelled") console.error("Load failed:", error);
        }
    }, [setBackEndState]);
}
