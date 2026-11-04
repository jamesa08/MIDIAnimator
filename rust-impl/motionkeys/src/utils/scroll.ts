import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// what a scroll over a graph does (the node_editor.scroll setting): auto pans for a trackpad's swipe and zooms for a
// mouse wheel, the others always zoom or always pan
export type ScrollMode = "auto" | "zoom" | "pan";

let mode: ScrollMode = "auto";
const takeSettings = (all: any) => {
    mode = all?.node_editor?.scroll ?? "auto";
};
invoke("get_settings")
    .then(takeSettings)
    .catch(() => {});
listen("settings_changed", (event: any) => takeSettings(event.payload));

// MARK: - Trackpad Or Wheel

// wheel events closer together than this (ms) are one gesture: a swipe and the coasting after it, or a wheel spun
const GESTURE_GAP = 200;

const mac = navigator.userAgent.includes("Mac");

// two fingers never move perfectly straight, a wheel only goes sideways with shift (macos turns shift+wheel sideways)
const sideways = (event: WheelEvent) => event.deltaX !== 0 && !event.shiftKey;

// what one event says about where it came from: true a trackpad, false a mouse wheel, null nothing
function fromTrackpad(event: WheelEvent): boolean | null {
    // lines or pages, only a wheel scrolls by those (firefox)
    if (event.deltaMode !== WheelEvent.DOM_DELTA_PIXEL) return false;
    if (sideways(event)) return true;
    if (event.deltaY === 0) return null;
    // whole notches of the old wheelDelta (windows, chromium). webkit gives a trackpad's as -3x its pixels
    const legacy = (event as any).wheelDeltaY;
    if (typeof legacy === "number" && legacy !== 0 && legacy % 120 === 0 && legacy !== -3 * event.deltaY) return false;
    // a mac wheel notch is a tenth of a 40px line in fixed point (4.000244px), a trackpad moves whole pixels
    if (mac && !Number.isInteger(event.deltaY)) return false;
    return true;
}

// the gesture going on, and what it was taken for
let gesture: { last: number; trackpad: boolean | null } = { last: -Infinity, trackpad: null };

// whether a scroll with no modifier pans the graph (true) or zooms it. a gesture keeps what its first telling event
// said so a swipe never flips between the two, only a sideways move turns a wheel into a trackpad partway
export function scrollPans(event: WheelEvent): boolean {
    if (mode !== "auto") return mode === "pan";
    if (event.timeStamp - gesture.last > GESTURE_GAP) gesture.trackpad = null;
    gesture.last = event.timeStamp;
    const trackpad = fromTrackpad(event);
    if (gesture.trackpad === null || (trackpad && sideways(event))) gesture.trackpad = trackpad ?? gesture.trackpad;
    // nothing up or down to zoom by (shift+wheel), it pans sideways
    return gesture.trackpad ?? event.deltaY === 0;
}
