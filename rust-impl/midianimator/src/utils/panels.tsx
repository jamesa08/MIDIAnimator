import { emitTo } from "@tauri-apps/api/event";
import { type BackgroundThrottlingPolicy, Window, getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { invoke } from "@tauri-apps/api/core";

// every dockable panel and the side of the main window it docks to
export const PANELS: Record<number, { name: string; side: "left" | "right" }> = {
    0: { name: "Nodes", side: "left" },
    1: { name: "Properties", side: "right" },
};

// tauri events sent to the main window while a panel is floating, coordinates are screen pixels
// { id, screenX, screenY }
export const PANEL_DRAG_EVENT = "panel-drag";
export const PANEL_DROP_EVENT = "panel-drop";
// { id }
export const PANEL_DOCK_EVENT = "panel-dock";
// same as NODE_DROP_EVENT's detail, with screenX/screenY instead of clientX/clientY
export const PANEL_NODE_DROP_EVENT = "panel-node-drop";

// docked panel width (w-60) plus how far past it a floating panel still docks
const DOCK_ZONE_WIDTH = 240 + 60;

export const panelLabel = (id: number) => `panel-${id}`;

export function sendToMain(event: string, payload: any) {
    return emitTo("main", event, payload).catch((e) => console.error(`Error sending ${event} to main: ${e}`));
}

// adds or removes a panel from the popped out list in the frontend state
export function withPoppedOut(prev: any, id: number, poppedOut: boolean) {
    const rest = prev.panelsPoppedOut.filter((p: number) => p !== id);
    return { ...prev, panelsPoppedOut: poppedOut ? [...rest, id] : rest };
}

// screen <-> client pixels for the calling window
async function windowOrigin() {
    const win = getCurrentWindow();
    return (await win.innerPosition()).toLogical(await win.scaleFactor());
}

export async function screenToClient(screenX: number, screenY: number) {
    const origin = await windowOrigin();
    return { x: screenX - origin.x, y: screenY - origin.y };
}

export async function clientToScreen(clientX: number, clientY: number) {
    const origin = await windowOrigin();
    return { x: clientX + origin.x, y: clientY + origin.y };
}

// true when a client point in the main window is over the panel's dock slot
export function inDockZone(id: number, clientX: number, clientY: number) {
    const content = document.querySelector(".content")?.getBoundingClientRect();
    if (!content || clientY < content.top || clientY > content.bottom) return false;
    if (PANELS[id]?.side === "right") return clientX >= content.right - DOCK_ZONE_WIDTH && clientX <= content.right;
    return clientX >= content.left && clientX <= content.left + DOCK_ZONE_WIDTH;
}

// moves a window to follow the cursor, at most once a frame and only once it's ready
export function windowMover(ready: Promise<Window>) {
    let win: Window | null = null;
    let pending: { x: number; y: number } | null = null;
    let scheduled = false;

    const flush = () => {
        scheduled = false;
        if (!win || !pending) return;
        win.setPosition(new LogicalPosition(pending.x, pending.y));
        pending = null;
    };

    ready.then((w) => {
        win = w;
        flush();
    });

    return (x: number, y: number) => {
        pending = { x, y };
        if (scheduled) return;
        scheduled = true;
        requestAnimationFrame(flush);
    };
}

// floating windows, created once by the main window and shown/hidden from then on (see src-tauri/src/ui/panels.rs)
const floatingWindows = new Map<string, Promise<WebviewWindow>>();

// creates a floating window hidden so it's already rendered when shown
function ensureFloatingWindow(label: string, options: ConstructorParameters<typeof WebviewWindow>[1]) {
    const existing = floatingWindows.get(label);
    if (existing) return existing;

    const created = (async () => {
        // still around after a main window reload, hide it to match the fresh state
        const old = await WebviewWindow.getByLabel(label);
        if (old) {
            await invoke("floating_window_set_shown", { label, shown: false });
            return old;
        }

        const win = new WebviewWindow(label, {
            decorations: false,
            visible: false,
            focus: false,
            backgroundThrottling: "disabled" as BackgroundThrottlingPolicy,
            useHttpsScheme: true,
            // clicks work without clicking the window first to focus it
            acceptFirstMouse: true,
            ...options,
        });
        await new Promise((resolve, reject) => {
            win.once("tauri://created", resolve);
            win.once("tauri://error", (e: any) => reject(new Error(`Error creating window ${label}: ${JSON.stringify(e.payload)}`)));
        });
        return win;
    })();

    floatingWindows.set(label, created);
    created.catch((e) => {
        console.error(e);
        floatingWindows.delete(label);
    });
    return created;
}

export function ensurePanelWindow(id: number, width: number, height: number) {
    return ensureFloatingWindow(panelLabel(id), {
        url: `/#/panel/${id}`,
        title: PANELS[id].name,
        width,
        height,
        minWidth: 160,
        minHeight: 120,
        resizable: true,
        backgroundColor: "#ffffff",
    });
}

// pops the panel's window out at a spot, resolves once it's showing
export async function showPanelWindow(id: number, x: number, y: number, width: number, height: number) {
    const win = await ensurePanelWindow(id, width, height);
    // size and place it while it's still invisible
    await win.setSize(new LogicalSize(width, height));
    await win.setPosition(new LogicalPosition(x, y));
    await invoke("floating_window_set_shown", { label: panelLabel(id), shown: true });
    return win;
}

export function hidePanelWindow(id: number) {
    floatingWindows
        .get(panelLabel(id))
        ?.then(() => invoke("floating_window_set_shown", { label: panelLabel(id), shown: false }))
        .catch(() => {});
}

// node preview that follows the cursor when dragging out of a floating panel, a window so it can leave the panel
export const DRAG_GHOST_LABEL = "drag-ghost";
// { nodeType: string | null, width }, width is the preview's container width in the panel
export const DRAG_GHOST_SET_EVENT = "drag-ghost-set";
// room around the node for its shadow
export const DRAG_GHOST_PAD = 12;

export function ensureDragGhostWindow() {
    return ensureFloatingWindow(DRAG_GHOST_LABEL, { url: "/#/drag-ghost", width: 480, height: 480, resizable: false, transparent: true, shadow: false });
}

// drives the drag ghost from any window, positions are the node's top left in screen pixels
export function startDragGhost(nodeType: string, width: number) {
    // set it now so the node is showing by the time the drag starts
    emitTo(DRAG_GHOST_LABEL, DRAG_GHOST_SET_EVENT, { nodeType, width });

    const ready = WebviewWindow.getByLabel(DRAG_GHOST_LABEL).then((win) => {
        if (!win) throw new Error("no drag ghost window");
        return win;
    });
    const move = windowMover(ready);
    let shown: Promise<unknown> | null = null;

    return {
        move: (x: number, y: number) => {
            move(x - DRAG_GHOST_PAD, y - DRAG_GHOST_PAD);
            // place it before the first show so it doesn't appear where the last drag ended
            shown ??= ready.then(async (win) => {
                await win.setPosition(new LogicalPosition(x - DRAG_GHOST_PAD, y - DRAG_GHOST_PAD));
                await invoke("floating_window_set_shown", { label: DRAG_GHOST_LABEL, shown: true });
            });
        },
        end: () => {
            (shown ?? Promise.resolve())
                .then(() => invoke("floating_window_set_shown", { label: DRAG_GHOST_LABEL, shown: false }))
                .then(() => emitTo(DRAG_GHOST_LABEL, DRAG_GHOST_SET_EVENT, { nodeType: null, width }))
                .catch((e) => console.error(`Error hiding drag ghost: ${e}`));
        },
    };
}
