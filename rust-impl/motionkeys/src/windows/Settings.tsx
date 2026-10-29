import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { THEMES } from "../utils/theme";
import KeymapEditor from "./KeymapEditor";
import { Button, Input, Switch } from "glisse";

// the page zooms, the same steps as the view menu's zoom in and out (ZOOM_STEPS in src-tauri/src/ui/windows.rs)
const ZOOMS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];

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
            <Switch id={id} checked={checked} onCheckedChange={onChange} />
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
            <Input id={id} type="number" min={1024} max={65535} value={draft} onValueChange={setDraft} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
        </Row>
    );
}

// a whole number setting, only saved on blur or enter once it's valid
function NumberInput({ id, label, description, value, min, onChange }: { id: string; label: string; description: string; value: number; min: number; onChange: (value: number) => void }) {
    const [draft, setDraft] = useState(String(value));
    useEffect(() => setDraft(String(value)), [value]);

    const number = Number(draft);
    const valid = Number.isInteger(number) && number >= min;

    // invalid input goes back to the saved value
    const commit = () => {
        if (!valid) setDraft(String(value));
        else if (number !== value) onChange(number);
    };

    return (
        <Row id={id} label={label} description={description}>
            <Input id={id} type="number" min={min} value={draft} onValueChange={setDraft} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
        </Row>
    );
}

// one choice from a list
function Choice({ id, label, description, value, options, onChange }: { id: string; label: string; description: string; value: string; options: [string, string][]; onChange: (value: string) => void }) {
    return (
        <Row id={id} label={label} description={description}>
            <select id={id} className="border border-black" value={value} onChange={(e) => onChange(e.target.value)}>
                {options.map(([value, name]) => (
                    <option key={value} value={value}>
                        {name}
                    </option>
                ))}
            </select>
        </Row>
    );
}

// the sections in the left column, the chosen one fills the right
const SECTIONS = [
    { id: "appearance", name: "Appearance" },
    { id: "panels", name: "Panels" },
    { id: "connection", name: "Blender connection" },
    { id: "keymap", name: "Keyboard Shortcuts" },
] as const;
type Section = (typeof SECTIONS)[number]["id"];

// settings live in the backend (src-tauri/src/settings.rs), changes are saved right away
function Settings() {
    const [settings, setSettings] = useState<any>(null);
    const [section, setSection] = useState<Section>("appearance");

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

    const name = SECTIONS.find((s) => s.id === section)!.name;

    return (
        <div className="settings h-screen flex text-sm select-none overflow-hidden">
            <nav className="settings-nav flex-none w-48 border-r border-black">
                {SECTIONS.map((s) => (
                    <button key={s.id} className={`settings-nav-item${s.id === section ? " active" : ""}`} onClick={() => setSection(s.id)}>
                        {s.name}
                    </button>
                ))}
            </nav>
            <div className="flex-auto min-w-0 flex flex-col">
                {section === "keymap" ? (
                    <KeymapEditor />
                ) : (
                    <section className="overflow-auto">
                        <div className="panel-header h-7 text-base border-b border-black flex items-center pl-2 pr-2">{name}</div>
                        <table className="w-full">
                            <tbody>
                                {section === "appearance" && <Choice id="appearance-theme" label="Theme" description="How the node graph looks." value={settings.appearance?.theme ?? "light"} options={THEMES.map((theme) => [theme, theme[0].toUpperCase() + theme.slice(1)])} onChange={(theme) => update("appearance.theme", theme)} />}
                                {section === "appearance" && <Choice id="appearance-zoom" label="UI Scale" description="" value={String(settings.appearance?.zoom ?? 1)} options={ZOOMS.map((zoom) => [String(zoom), `${Math.round(zoom * 100)}%`])} onChange={(zoom) => update("appearance.zoom", Number(zoom))} />}
                                {section === "panels" && <Toggle id="panels-hide-when-inactive" label="Hide floating panels in the background" description="Floating panels hide while MotionKeys isn't the active app, like Photoshop." checked={settings.panels?.hide_when_inactive ?? false} onChange={(checked) => update("panels.hide_when_inactive", checked)} />}
                                {section === "connection" && <PortInput id="ipc-port" label="Port" description="Port the Blender add-on connects to (1024 to 65535). Set the same port in the add-on's panel. Applies after restarting MotionKeys." value={typeof settings.ipc?.port === "number" ? settings.ipc.port : 6577} onChange={(port) => update("ipc.port", port)} />}
                                {section === "connection" && <NumberInput id="keyframe-import-limit" label="Keyframe import limit" description="" value={typeof settings.blender?.keyframe_import_limit === "number" ? settings.blender.keyframe_import_limit : 5000} min={0} onChange={(limit) => update("blender.keyframe_import_limit", limit)} />}
                                {section === "connection" && (
                                    <Row id="save-diagnostics" label="" description="">
                                        <Button id="save-diagnostics" onClick={() => invoke("save_diagnostics").catch(console.error)}>
                                            Save Diagnostics…
                                        </Button>
                                    </Row>
                                )}
                            </tbody>
                        </table>
                    </section>
                )}
            </div>
        </div>
    );
}

export default Settings;
