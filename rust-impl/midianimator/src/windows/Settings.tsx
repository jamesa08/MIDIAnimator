import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// one table row, label on the left and the control on the right. the description shows on hover
function Row({ id, label, description, children }: { id: string; label: string; description: string; children: React.ReactNode }) {
    return (
        <tr title={description}>
            <td className="w-1/2 px-2 py-1">
                <label htmlFor={id}>{label}</label>
            </td>
            <td className="px-2 py-1">{children}</td>
        </tr>
    );
}

// one on/off setting
function Toggle({ id, label, description, checked, onChange }: { id: string; label: string; description: string; checked: boolean; onChange: (checked: boolean) => void }) {
    return (
        <Row id={id} label={label} description={description}>
            <input id={id} type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
        </Row>
    );
}

// ipc port field, only saved on blur or enter once it's a valid port
function PortInput({ id, label, description, value, onChange }: { id: string; label: string; description: string; value: number; onChange: (value: number) => void }) {
    const [draft, setDraft] = useState(String(value));
    useEffect(() => setDraft(String(value)), [value]);

    const port = Number(draft);
    const valid = Number.isInteger(port) && port >= 1024 && port <= 65535;

    // invalid input goes back to the saved port
    const commit = () => {
        if (!valid) setDraft(String(value));
        else if (port !== value) onChange(port);
    };

    return (
        <Row id={id} label={label} description={description}>
            <input id={id} type="number" min={1024} max={65535} className="w-20 border border-black px-1" value={draft} onChange={(e) => setDraft(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
        </Row>
    );
}

// settings live in the backend (src-tauri/src/settings.rs), changes are saved right away
function Settings() {
    const [settings, setSettings] = useState<any>(null);

    useEffect(() => {
        invoke("get_settings").then(setSettings);
        const unlisten = listen("settings_changed", (event: any) => setSettings(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);

    // settings are set by dotted path, e.g. "panels.hide_when_inactive"
    const update = (path: string, value: any) => {
        invoke("set_setting", { path, value }).catch((e) => console.error(`Error saving setting ${path}: ${e}`));
    };

    if (!settings) return null;

    return (
        <div className="settings text-sm select-none">
            <section className="mb-2">
                <div className="panel-header h-7 text-base border-b border-black flex items-center pl-2 pr-2">Panels</div>
                <table className="w-full">
                    <tbody>
                        <Toggle id="panels-hide-when-inactive" label="Hide floating panels in the background" description="Floating panels hide while MotionKeys isn't the active app, like Photoshop." checked={settings.panels?.hide_when_inactive ?? false} onChange={(checked) => update("panels.hide_when_inactive", checked)} />
                    </tbody>
                </table>
            </section>

            <section className="mb-2">
                <div className="panel-header h-7 text-base border-t border-b border-black flex items-center pl-2 pr-2">Blender connection</div>
                <table className="w-full">
                    <tbody>
                        <PortInput id="ipc-port" label="Port" description="Port the Blender add-on connects to (1024 to 65535). Set the same port in the add-on's panel. Applies after restarting MotionKeys." value={typeof settings.ipc?.port === "number" ? settings.ipc.port : 6577} onChange={(port) => update("ipc.port", port)} />
                    </tbody>
                </table>
            </section>
        </div>
    );
}

export default Settings;
