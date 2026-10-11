import React, { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { KeyCommand, KeyScope, Keymap, MODIFIERS, MOUSE_BUTTONS, Modifier, combo, formatCombo, keyName, modifierName, pushModal, useKeymapData } from "../utils/keymap";
import { KEYBOARD_HEIGHT, KEYBOARD_WIDTH, KeyCap, keyboardLayout } from "./keyboardLayout";

// a command by its scope
type CommandRef = { scope: string; command: string };
// one of a command's keys
type Chip = CommandRef & { index: number };
// a command waiting for a key to be pressed, replacing one of its keys or adding one (index null)
type Capture = CommandRef & { index: number | null };
// a scope's keys before an edit, undo puts them back
type Snapshot = { scope: string; keys: Record<string, string[]> };
// a command dragged onto the keyboard
type Drag = CommandRef & { name: string; x: number; y: number };

// every set of modifiers, fewest first, for the selected key's list
const MODIFIER_SETS: Modifier[][] = Array.from({ length: 16 }, (_, bits) => MODIFIERS.filter((_, i) => bits & (1 << i))).sort((a, b) => a.length - b.length || MODIFIERS.indexOf(a[0]) - MODIFIERS.indexOf(b[0]) || MODIFIERS.indexOf(a[1]) - MODIFIERS.indexOf(b[1]));

const button = "px-2 h-6 border border-black text-sm hover:bg-zinc-100 disabled:opacity-40 disabled:hover:bg-transparent";

const findCommand = (keymap: Keymap, ref: CommandRef) => keymap.scopes.find((s) => s.id === ref.scope)?.commands.find((c) => c.id === ref.command);

// the command a key runs in `scope` with `modifiers` held, a hold command shows on its key with no modifiers
function commandOn(scope: KeyScope | null, cap: KeyCap, modifiers: Modifier[]): KeyCommand | null {
    if (!scope || !cap.name) return null;
    if (cap.modifier) return scope.commands.find((c) => c.hold && c.keys.includes(cap.name!)) ?? null;
    const key = combo(modifiers, cap.name);
    return scope.commands.find((c) => (c.hold ? modifiers.length === 0 && c.keys.includes(cap.name!) : c.keys.includes(key))) ?? null;
}

// a key's fill: purple for an app command, green for the chosen area or modal's, split when it has both (like premiere)
function capFill(app: KeyCommand | null, area: KeyCommand | null): React.CSSProperties {
    if (app && area) return { background: "linear-gradient(135deg, var(--keymap-app) 50%, var(--keymap-area) 50%)", color: "white" };
    if (app) return { background: "var(--keymap-app)", color: "white" };
    if (area) return { background: "var(--keymap-area)", color: "white" };
    return {};
}

// the shortcut editor in the settings window, laid out like premiere's: presets and the scope on top, the keyboard with what
// each key does, then the commands with their keys and what the selected key does with each set of modifiers
function KeymapEditor() {
    const keymap = useKeymapData();
    const [scopeId, setScopeId] = useState("app");
    const [search, setSearch] = useState("");
    const [selected, setSelected] = useState<CommandRef | null>(null);
    const [chip, setChip] = useState<Chip | null>(null);
    const [capture, setCapture] = useState<Capture | null>(null);
    const [selectedKey, setSelectedKey] = useState<string | null>(null);
    // modifiers clicked on the keyboard and held on the real one, the keyboard shows what keys do with them
    const [toggled, setToggled] = useState<Modifier[]>([]);
    const [pressed, setPressed] = useState<Modifier[]>([]);
    const [undoStack, setUndoStack] = useState<Snapshot[]>([]);
    // the name a preset is being saved as
    const [naming, setNaming] = useState<string | null>(null);
    const [drag, setDrag] = useState<Drag | null>(null);

    const modifiers = MODIFIERS.filter((m) => toggled.includes(m) || pressed.includes(m));
    const mac = keymap?.platform === "mac";
    const caps = useMemo(() => keyboardLayout(mac), [mac]);

    // read by the capture and drag listeners
    const latest = useRef({ keymap, modifiers, capture });
    latest.current = { keymap, modifiers, capture };

    useEffect(() => {
        const update = (event: KeyboardEvent) => setPressed(MODIFIERS.filter((m) => ({ Ctrl: event.ctrlKey, Alt: event.altKey, Shift: event.shiftKey, Cmd: event.metaKey })[m]));
        const clear = () => setPressed([]);
        window.addEventListener("keydown", update, true);
        window.addEventListener("keyup", update, true);
        window.addEventListener("blur", clear);
        return () => {
            window.removeEventListener("keydown", update, true);
            window.removeEventListener("keyup", update, true);
            window.removeEventListener("blur", clear);
        };
    }, []);

    // sets a command's keys, what its scope had before goes on the undo stack
    const setKeys = (ref: CommandRef, keys: string[]) => {
        const scope = latest.current.keymap?.scopes.find((s) => s.id === ref.scope);
        if (!scope) return;
        setUndoStack((stack) => [...stack, { scope: scope.id, keys: Object.fromEntries(scope.commands.map((c) => [c.id, c.keys])) }]);
        invoke("keymap_set", { scope: ref.scope, command: ref.command, keys }).catch((e) => console.error(`Error setting keys: ${e}`));
    };

    // the commands whose keys changed since the last edit get them back
    const undo = async () => {
        const snapshot = undoStack[undoStack.length - 1];
        const scope = keymap?.scopes.find((s) => s.id === snapshot?.scope);
        if (!snapshot || !scope) return;
        setUndoStack((stack) => stack.slice(0, -1));
        for (const command of scope.commands.filter((c) => !c.fixed && snapshot.keys[c.id] && snapshot.keys[c.id].join(" ") !== c.keys.join(" "))) {
            await invoke("keymap_set", { scope: scope.id, command: command.id, keys: snapshot.keys[command.id] }).catch((e) => console.error(`Error undoing keys: ${e}`));
        }
    };

    const clear = () => {
        const command = chip && keymap && findCommand(keymap, chip);
        if (!chip || !command) return;
        setKeys(
            chip,
            command.keys.filter((_, i) => i !== chip.index)
        );
        setChip(null);
    };

    // a key pressed (or a key on the drawn keyboard clicked) while capturing becomes the command's key. a modal's command can
    // take a mouse button pressed on the empty key. clicking anywhere else stops capturing
    useEffect(() => {
        if (!capture) return;
        const finish = (key: string) => {
            const { keymap, capture } = latest.current;
            setCapture(null);
            const command = keymap && capture && findCommand(keymap, capture);
            if (!capture || !command) return;
            const keys = [...command.keys];
            if (capture.index === null) keys.push(key);
            else keys[capture.index] = key;
            setKeys(capture, keys);
        };
        return pushModal(
            null,
            {},
            {
                raw: (event) => {
                    const { keymap, capture, modifiers } = latest.current;
                    const command = keymap && capture && findCommand(keymap, capture);
                    if (!keymap || !capture || !command) return false;
                    const modal = keymap.scopes.find((s) => s.id === capture.scope)?.kind === "modal";
                    if (event instanceof KeyboardEvent) {
                        const modifier = modifierName(event.code);
                        const name = keyName(event.code);
                        if (command.hold) {
                            if (modifier || name) finish(modifier ?? name!);
                            return true;
                        }
                        // a modifier on its own waits for the key
                        if (!name) return false;
                        finish(combo(event, name));
                        return true;
                    }
                    const target = event.target as Element;
                    const mouse = MOUSE_BUTTONS[event.button];
                    if (modal && !command.hold && mouse && target.closest("[data-capture]")) {
                        finish(combo(event, mouse));
                        return true;
                    }
                    const cap = target.closest<HTMLElement>("[data-key]");
                    if (cap && (command.hold || !cap.dataset.modifier)) {
                        finish(command.hold ? cap.dataset.key! : combo(modifiers, cap.dataset.key!));
                        return true;
                    }
                    // a modifier on the drawn keyboard still toggles
                    if (cap) return false;
                    setCapture(null);
                    return false;
                },
            }
        );
    }, [capture]);

    // a command dragged by its name onto a key on the keyboard gets that key (with the shown modifiers) added
    const startDrag = (event: React.PointerEvent, scope: KeyScope, command: KeyCommand) => {
        if (event.button !== 0 || command.fixed) return;
        const startX = event.clientX;
        const startY = event.clientY;
        let dragging = false;
        const handleMove = (e: PointerEvent) => {
            if (!dragging && Math.hypot(e.clientX - startX, e.clientY - startY) < 4) return;
            dragging = true;
            setDrag({ scope: scope.id, command: command.id, name: command.name, x: e.clientX, y: e.clientY });
        };
        const cleanup = () => {
            endModal();
            setDrag(null);
            window.removeEventListener("pointermove", handleMove);
            window.removeEventListener("pointerup", handleUp);
        };
        const handleUp = (e: PointerEvent) => {
            cleanup();
            if (!dragging) return;
            const cap = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-key]");
            const current = latest.current.keymap && findCommand(latest.current.keymap, { scope: scope.id, command: command.id });
            if (!cap || !current || (cap.dataset.modifier && !current.hold)) return;
            const key = current.hold ? cap.dataset.key! : combo(latest.current.modifiers, cap.dataset.key!);
            if (!current.keys.includes(key)) setKeys({ scope: scope.id, command: command.id }, [...current.keys, key]);
        };
        // the drag takes the keys, cancelling (escape) drops nothing
        const endModal = pushModal("drag", { cancel: cleanup });
        window.addEventListener("pointermove", handleMove);
        window.addEventListener("pointerup", handleUp);
    };

    const savePreset = () => {
        const name = naming?.trim();
        setNaming(null);
        if (!name) return;
        setUndoStack([]);
        invoke("keymap_save_as", { name }).catch((e) => console.error(`Error saving preset: ${e}`));
    };

    const selectPreset = (name: string) => {
        setUndoStack([]);
        invoke("keymap_select", { name }).catch((e) => console.error(`Error selecting preset: ${e}`));
    };

    const deletePreset = () => {
        if (!keymap || keymap.builtin) return;
        setUndoStack([]);
        invoke("keymap_delete", { name: keymap.preset }).catch((e) => console.error(`Error deleting preset: ${e}`));
    };

    // every scope's commands and keys as text
    const copyToClipboard = () => {
        if (!keymap) return;
        const text = keymap.scopes.map((scope) => [scope.name, ...scope.commands.map((c) => `\t${c.name}\t${c.keys.map((k) => formatCombo(k, keymap.platform)).join(", ")}`)].join("\n")).join("\n\n");
        navigator.clipboard.writeText(text).catch((e) => console.error(`Error copying keymap: ${e}`));
    };

    if (!keymap) return null;

    const scope = keymap.scopes.find((s) => s.id === scopeId) ?? keymap.scopes[0];
    const app = keymap.scopes.find((s) => s.kind === "app") ?? null;
    // app commands don't run during a modal, its keyboard only shows its own
    const appLayer = scope.kind === "modal" ? null : app;
    const areaLayer = scope.kind === "app" ? null : scope;

    // the chosen scope's commands, or every scope's that match the search (all of a scope's when its name matches)
    const query = search.trim().toLowerCase();
    const matches = (c: KeyCommand) => c.name.toLowerCase().includes(query) || c.keys.some((k) => formatCombo(k, keymap.platform).toLowerCase().includes(query));
    const groups = query ? keymap.scopes.map((s) => ({ scope: s, commands: s.name.toLowerCase().includes(query) ? s.commands : s.commands.filter(matches) })).filter((g) => g.commands.length > 0) : [{ scope, commands: scope.commands }];

    const selectCommand = (ref: CommandRef) => {
        setSelected(ref);
        setScopeId(ref.scope);
    };

    const selectedCap = selectedKey ? caps.find((c) => c.name === selectedKey && !c.modifier) : undefined;

    return (
        <div className="keymap-editor h-full flex flex-col min-h-0">
            {/* presets and the scope shown */}
            <div className="flex-none flex items-center gap-2 px-2 h-10 border-b border-black">
                <span>Preset</span>
                {naming === null ? (
                    <>
                        <select className="border border-black h-6" value={keymap.preset} onChange={(e) => selectPreset(e.target.value)}>
                            {keymap.presets.map((name) => (
                                <option key={name} value={name}>
                                    {name}
                                </option>
                            ))}
                        </select>
                        <button className={button} onClick={() => setNaming("")}>
                            Save As...
                        </button>
                        <button className={button} onClick={copyToClipboard}>
                            Copy To Clipboard
                        </button>
                        <button className={button} disabled={keymap.builtin} onClick={deletePreset}>
                            Delete
                        </button>
                    </>
                ) : (
                    <>
                        <input
                            className="border border-black h-6 px-1 w-48 select-text"
                            autoFocus
                            value={naming}
                            onChange={(e) => setNaming(e.target.value)}
                            onKeyDown={(e) => {
                                if (e.key === "Enter") savePreset();
                                else if (e.key === "Escape") setNaming(null);
                            }}
                        />
                        <button className={`${button} bg-black text-white hover:bg-zinc-800`} onClick={savePreset}>
                            Save
                        </button>
                        <button className={button} onClick={() => setNaming(null)}>
                            Cancel
                        </button>
                    </>
                )}
                <span className="ml-auto">Commands</span>
                <select className="border border-black h-6" value={scope.id} onChange={(e) => setScopeId(e.target.value)}>
                    {keymap.scopes.map((s) => (
                        <option key={s.id} value={s.id}>
                            {s.name}
                        </option>
                    ))}
                </select>
            </div>

            {/* the keyboard, each key showing what it does with the modifiers held */}
            <div className="flex-none p-2 border-b border-black">
                <div className="relative w-full" style={{ aspectRatio: `${KEYBOARD_WIDTH} / ${KEYBOARD_HEIGHT}` }}>
                    {caps.map((cap, i) => {
                        const appCommand = commandOn(appLayer, cap, modifiers);
                        const areaCommand = commandOn(areaLayer, cap, modifiers);
                        const command = areaCommand ?? appCommand;
                        const active = cap.modifier && modifiers.includes(cap.modifier);
                        const fill = active ? { background: "black", color: "white" } : !cap.name ? { opacity: 0.35 } : capFill(appCommand, areaCommand);
                        return (
                            <div
                                key={i}
                                data-key={cap.name ?? undefined}
                                data-modifier={cap.modifier}
                                className="absolute p-[2px]"
                                style={{ left: `${(cap.x / KEYBOARD_WIDTH) * 100}%`, top: `${(cap.y / KEYBOARD_HEIGHT) * 100}%`, width: `${(cap.w / KEYBOARD_WIDTH) * 100}%`, height: `${(cap.h / KEYBOARD_HEIGHT) * 100}%` }}
                                onClick={() => {
                                    if (!cap.name) return;
                                    if (cap.modifier) setToggled((t) => (t.includes(cap.modifier!) ? t.filter((m) => m !== cap.modifier) : [...t, cap.modifier!]));
                                    else setSelectedKey(cap.name);
                                }}
                            >
                                <div className={`h-full w-full border border-black flex flex-col justify-between overflow-hidden px-[3px] py-[1px] text-[10px] leading-[11px] ${selectedKey && selectedKey === cap.name && !cap.modifier ? "outline outline-2 outline-black -outline-offset-[3px]" : ""}`} style={fill}>
                                    <span className="line-clamp-2">{command?.name}</span>
                                    <span className="truncate">{cap.label}</span>
                                </div>
                            </div>
                        );
                    })}
                </div>
            </div>

            <div className="flex-auto flex min-h-0">
                {/* the commands and their keys */}
                <div className="flex-auto min-w-0 flex flex-col border-r border-black">
                    <div className="flex-none flex items-center gap-1 px-2 h-8 border-b border-black">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={2} stroke="currentColor" className="size-4 flex-none">
                            <circle cx="10.5" cy="10.5" r="6" />
                            <path strokeLinecap="round" d="M15 15l5 5" />
                        </svg>
                        <input className="flex-auto min-w-0 border border-black h-6 px-1 select-text" value={search} onChange={(e) => setSearch(e.target.value)} />
                    </div>
                    <div className="flex-auto overflow-auto">
                        <table className="w-full table-fixed">
                            <thead>
                                <tr className="text-left border-b border-black">
                                    <th className="w-2/5 px-2 font-normal">Command</th>
                                    <th className="px-2 font-normal">Shortcut</th>
                                </tr>
                            </thead>
                            <tbody>
                                {groups.map(({ scope: s, commands }) => (
                                    <React.Fragment key={s.id}>
                                        {query && (
                                            <tr className="panel-header">
                                                <td colSpan={2} className="px-2 border-b border-black">
                                                    {s.name}
                                                </td>
                                            </tr>
                                        )}
                                        {commands.map((c) => {
                                            const isSelected = selected?.scope === s.id && selected.command === c.id;
                                            const capturing = capture?.scope === s.id && capture.command === c.id ? capture : null;
                                            return (
                                                <tr key={c.id} className={isSelected ? "bg-zinc-100" : ""} onClick={() => selectCommand({ scope: s.id, command: c.id })}>
                                                    <td className={`px-2 py-0.5 truncate ${c.fixed ? "text-zinc-400" : "cursor-grab"}`} onPointerDown={(e) => startDrag(e, s, c)}>
                                                        {c.name}
                                                    </td>
                                                    <td
                                                        className="px-2 py-0.5"
                                                        onClick={(e) => {
                                                            if (!c.fixed && !(e.target as Element).closest("[data-chip]")) setCapture({ scope: s.id, command: c.id, index: null });
                                                        }}
                                                    >
                                                        <div className="flex flex-wrap gap-1 min-h-[18px]">
                                                            {c.keys.map((key, index) => {
                                                                if (capturing?.index === index) return <span key={index} data-chip data-capture className="px-1 min-w-[32px] h-[18px] border border-dashed border-black text-xs" />;
                                                                const chipSelected = chip?.scope === s.id && chip.command === c.id && chip.index === index;
                                                                return (
                                                                    <span
                                                                        key={index}
                                                                        data-chip
                                                                        className={`px-1 h-[18px] leading-[16px] border text-xs whitespace-nowrap ${c.fixed ? "border-zinc-300 text-zinc-400" : chipSelected ? "border-black bg-black text-white" : "border-black"}`}
                                                                        onClick={() => {
                                                                            if (!c.fixed) setChip({ scope: s.id, command: c.id, index });
                                                                        }}
                                                                        onDoubleClick={() => {
                                                                            if (!c.fixed) setCapture({ scope: s.id, command: c.id, index });
                                                                        }}
                                                                    >
                                                                        {formatCombo(key, keymap.platform)}
                                                                    </span>
                                                                );
                                                            })}
                                                            {capturing?.index === null && <span data-chip data-capture className="px-1 min-w-[32px] h-[18px] border border-dashed border-black text-xs" />}
                                                        </div>
                                                    </td>
                                                </tr>
                                            );
                                        })}
                                    </React.Fragment>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </div>

                {/* what the selected key does with each set of modifiers */}
                <div className="flex-none w-72 flex flex-col">
                    <div className="flex-none px-2 h-8 flex items-center border-b border-black">Key: {selectedCap ? formatCombo(selectedCap.name!, keymap.platform) : ""}</div>
                    <div className="flex-auto overflow-auto">
                        <table className="w-full table-fixed">
                            <thead>
                                <tr className="text-left border-b border-black">
                                    <th className="w-24 px-2 font-normal">Modifiers</th>
                                    <th className="px-2 font-normal">Command</th>
                                </tr>
                            </thead>
                            <tbody>
                                {selectedCap &&
                                    MODIFIER_SETS.map((set) => {
                                        const appCommand = commandOn(appLayer, selectedCap, set);
                                        const areaCommand = commandOn(areaLayer, selectedCap, set);
                                        const command = areaCommand ?? appCommand;
                                        return (
                                            <tr key={set.join("+")}>
                                                <td className="px-2">{set.length === 0 ? "None" : formatCombo(combo(set, ""), keymap.platform).replace(/\+$/, "")}</td>
                                                <td className="px-2 truncate">
                                                    {command && <span className="inline-block size-2 mr-1 align-middle" style={{ background: areaCommand ? "var(--keymap-area)" : "var(--keymap-app)" }} />}
                                                    {command?.name}
                                                </td>
                                            </tr>
                                        );
                                    })}
                            </tbody>
                        </table>
                    </div>
                    <div className="flex-none flex justify-end gap-2 p-2 border-t border-black">
                        <button className={button} disabled={undoStack.length === 0} onClick={undo}>
                            Undo
                        </button>
                        <button className={button} disabled={!chip} onClick={clear}>
                            Clear
                        </button>
                    </div>
                </div>
            </div>

            {drag && (
                <div className="fixed pointer-events-none z-50 px-1 border border-black bg-white text-sm" style={{ left: drag.x + 8, top: drag.y + 8 }}>
                    {drag.name}
                </div>
            )}
        </div>
    );
}

export default KeymapEditor;
