import { invoke } from "@tauri-apps/api/core";
import { useCallback, useRef } from "react";
import { useStateContext } from "../contexts/StateContext";
import { PANELS, panelWindowFrame } from "./panels";
import { queued, takeState } from "./graphOps";
import { showStatus } from "./status";
import { askUnsaved } from "../components/UnsavedChangesModal";

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
            // the tab whose layout was just sent, the backend's tab on screen may be another one by now. after the tab's
            // edits still on their way, so the file has them. not in the queue itself, edits don't wait on the save dialog
            const saved = tab ?? shown;
            await queued(saved, async () => {});
            const path = await invoke<string>("save_project", { tab: saved, saveAs });
            showStatus(`Saved "${fileName(path)}"`);
            return path;
        },
        [setTabLayout]
    );
}

// asks for a project file and opens it in a tab of its own, or the recent project at the path `recent` (File > Open
// Recent), see pick_project and try_load_project in src-tauri/src/state/mod.rs. a file a tab has open is read again in
// that tab's place, one with unsaved changes is shown and asks to save them first. cancelling, or cancelling a save
// dialog, opens nothing
export function useOpenFile() {
    const { setBackEndState } = useStateContext();
    const saveTab = useSaveTab();
    return useCallback(
        async (recent?: string) => {
            try {
                const path = recent ?? (await invoke<{ path: string }>("pick_project", { recent: null })).path;
                // the backend checks for unsaved changes as it opens the file, it asks again while the tab has some (MCP
                // can edit it while the prompt is up or after a save)
                let discard = false;
                let closed: string | null = null;
                for (;;) {
                    const { state, unsaved_tab } = await invoke<{ state: any; unsaved_tab: string | null }>("try_load_project", { path, discard });
                    if (state) {
                        setBackEndState((s: any) => takeState(s, state));
                        showStatus(`Opened "${fileName(path)}"`);
                        return;
                    }
                    if (!unsaved_tab || unsaved_tab === closed) return;
                    // a tab closed since has nothing to lose, the file opens on the next try
                    const shownAt = await invoke<number | null>("switch_active_instance", { id: unsaved_tab });
                    if (shownAt == null) {
                        closed = unsaved_tab;
                        continue;
                    }
                    const answer = await askUnsaved(unsaved_tab, shownAt);
                    if (answer === "cancel") return;
                    if (answer === "save") await saveTab(unsaved_tab);
                    else discard = true;
                }
            } catch (error) {
                if (error !== "Load cancelled" && error !== "Save cancelled") console.error("Load failed:", error);
            }
        },
        [setBackEndState, saveTab]
    );
}
