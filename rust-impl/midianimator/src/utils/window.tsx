import { WebviewWindow, currentMonitor } from "@tauri-apps/api/window";

export async function safeWindowPosition(x: number, y: number, width: number, height: number) {
    const monitor = await currentMonitor();
    const screenWidth = monitor?.size.width ?? 1920;
    const screenHeight = monitor?.size.height ?? 1080;

    return {
        x: Math.max(0, Math.min(x, screenWidth - width)),
        y: Math.max(0, Math.min(y, screenHeight - height)),
    };
}
