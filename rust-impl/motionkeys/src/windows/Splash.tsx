import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
const appWindow = getCurrentWebviewWindow()

const img = new Image();
img.src = "splash.png";

const setup = async () => {
    // set the splash image as the background and wait for the image to load before showing the window to avoid white
    document.body.style.backgroundImage = `url("splash.png")`;
    
    await appWindow.show();
    
    listen("splash_progress", (event: any) => {
        document.getElementById("status")!.textContent = event.payload as string;
    });

    // get build info and display it 
    invoke("get_build_info").then((res: any) => {
        const [version, hash] = res;
        document.getElementById("version")!.textContent = `version: ${version}`;
        document.getElementById("hash")!.textContent = `hash: ${hash}`;
    });
};

img.onload = setup;
img.onerror = setup;
