import { WebviewWindow, getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export async function safeWindowPosition(x: number, y: number, width: number, height: number) {
    const monitor = await getCurrentWebviewWindow();
    const size = await monitor?.size();
    const screenWidth = size?.width ?? 1920;
    const screenHeight = size?.height ?? 1080;
        
    return {
        x: Math.max(0, Math.min(x, screenWidth - width)),
        y: Math.max(0, Math.min(y, screenHeight - height)),
    };
}
