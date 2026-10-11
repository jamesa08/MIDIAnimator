// stands in for the rust backend: answers invokes with what the real commands gave for the fixture project
// (backend.json, written by src-tauri/tests/smoke_backend_test.rs) and runs events like tauri does
import { readFileSync } from "fs";
import { join } from "path";
import type { Page } from "@playwright/test";

const BACKEND_PATH = join(__dirname, "backend.json");

export type Backend = Record<string, any>;

// the dumped commands, plus the node specs file the panel windows read themselves
export function loadBackend(): Backend {
    let backend: Backend;
    try {
        backend = JSON.parse(readFileSync(BACKEND_PATH, "utf8"));
    } catch {
        throw new Error(`no ${BACKEND_PATH}, run "npm run smoke" (or "cargo test --test smoke_backend_test" in src-tauri) first`);
    }
    backend.default_nodes_json = readFileSync(join(__dirname, "../src-tauri/src/configs/default_nodes.json"), "utf8");
    return backend;
}

// runs in the page before the app's scripts. `label` is the tauri window the page is in
function install({ backend, label }: { backend: Backend; label: string }) {
    const w = window as any;
    const callbacks = new Map<number, (data: any) => void>();
    const listeners = new Map<string, number[]>();
    // every invoke the page made, for tests to check
    w.__smokeCalls = [] as { cmd: string; args: any }[];

    const run = (id: number, data: any) => callbacks.get(id)?.(data);
    const emit = (event: string, payload: any) => {
        for (const id of listeners.get(event) ?? []) run(id, { event, id, payload });
    };
    w.__smokeEmit = emit;

    // window plugin calls that have to give something shaped right
    const WINDOW: Record<string, any> = {
        "plugin:window|scale_factor": 1,
        "plugin:window|inner_position": { x: 0, y: 0 },
        "plugin:window|outer_position": { x: 0, y: 0 },
        "plugin:window|inner_size": { width: 1280, height: 800 },
        "plugin:window|outer_size": { width: 1280, height: 800 },
        "plugin:window|is_visible": true,
        "plugin:window|is_focused": true,
        "plugin:window|get_all_windows": [],
    };
    // graph edits are taken without changing anything, the app gets its graph back
    const EDITS = ["graph_apply", "graph_cut", "graph_paste"];
    const state = backend.get_state;

    const handle = (cmd: string, args: any) => {
        switch (cmd) {
            case "plugin:event|listen": {
                listeners.set(args.event, [...(listeners.get(args.event) ?? []), args.handler]);
                return args.handler;
            }
            case "plugin:event|unlisten": {
                listeners.set(
                    args.event,
                    (listeners.get(args.event) ?? []).filter((id) => id !== args.eventId)
                );
                return null;
            }
            case "plugin:event|emit":
            case "plugin:event|emit_to":
                emit(args.event, args.payload);
                return null;
            case "plugin:fs|read_text_file":
                return Array.from(new TextEncoder().encode(backend.default_nodes_json));
            case "ready":
                return state;
        }
        // the results' node, by value
        if (cmd === "graph_value_curves") {
            const outputs = JSON.stringify(args.outputs);
            const node = Object.keys(state.executed_results).find((node) => JSON.stringify(state.executed_results[node]) === outputs);
            return node ? backend.graph_value_curves[node] : [];
        }
        if (cmd === "graph_curves") return args.nodes.map((node: string) => backend.graph_curves[node] ?? { node, channels: [] });
        if (EDITS.includes(cmd)) {
            // a selection is kept, so a test can select something and then act on it
            for (const op of args.ops ?? []) {
                if (op.op !== "select") continue;
                for (const node of state.rf_instance.nodes) {
                    node.selected = op.nodes.includes(node.id);
                    const sockets = (side: string) => op.sockets.filter((s: any) => s.node === node.id && s.side === side).map((s: any) => s.socket);
                    if (op.sockets.some((s: any) => s.node === node.id)) node.selectedSockets = { inputs: sockets("inputs"), outputs: sockets("outputs") };
                    else delete node.selectedSockets;
                }
                for (const edge of state.rf_instance.edges) edge.selected = op.edges.includes(edge.id);
            }
            return { tab: args.tab, graph_rev: state.graph_rev, rf_instance: state.rf_instance, added: [] };
        }
        if (cmd in WINDOW) return WINDOW[cmd];
        if (cmd in backend) return backend[cmd];
        // everything else (edits, saving, window management) does nothing
        return null;
    };

    w.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label }, currentWebview: { windowLabel: label, label } },
        invoke: async (cmd: string, args: any) => {
            w.__smokeCalls.push({ cmd, args });
            // a copy, the app may change what it gets
            return structuredClone(handle(cmd, args ?? {}));
        },
        transformCallback: (callback: (data: any) => void, once = false) => {
            const id = crypto.getRandomValues(new Uint32Array(1))[0];
            callbacks.set(id, (data) => {
                if (once) callbacks.delete(id);
                callback?.(data);
            });
            return id;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        convertFileSrc: (path: string, protocol = "asset") => `${protocol}://localhost/${encodeURIComponent(path)}`,
    };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: (_event: string, id: number) => callbacks.delete(id) };
}

// errors the page logged or threw, collected from before it loads
export function collectErrors(page: Page): string[] {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(`uncaught: ${error.stack ?? error.message}`));
    page.on("console", (message) => {
        if (message.type() === "error") errors.push(`console.error: ${message.text()}`);
    });
    return errors;
}

// opens `path` (index.html's hash route, or another page) as tauri window `label` with the fake backend
export async function openWindow(page: Page, backend: Backend, path: string, label = "main") {
    await page.addInitScript(install, { backend, label });
    await page.goto(path);
}

// sends a backend event to the page
export async function emitEvent(page: Page, event: string, payload: any) {
    await page.evaluate(([event, payload]) => (window as any).__smokeEmit(event, payload), [event, payload] as const);
}
