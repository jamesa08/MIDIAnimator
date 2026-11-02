// loads every window of the app in chromium (the engine windows' webview2 runs) with the fixture project, pokes at
// it, and fails on anything thrown or logged as an error. react's dev mode mounts each effect twice, so a bad cleanup
// shows up just by mounting
import { test, expect, type Page } from "@playwright/test";
import { collectErrors, emitEvent, loadBackend, openWindow } from "./backend";

const backend = loadBackend();

// the fixture with a layout of its own
function withLayout(layout: any) {
    return { ...backend, get_state: { ...backend.get_state, layout } };
}

// every panel docked, so they all draw in the main window
const ALL_DOCKED = { panelsShown: [0, 1, 2], panelsPoppedOut: [], panelSides: {}, sidesHidden: [], floating: {} };

let errors: string[];
test.beforeEach(({ page }) => {
    errors = collectErrors(page);
});

// effects, timers and the second strict mode mount get to run
async function settle(page: Page) {
    await page.waitForLoadState("networkidle");
    await page.waitForTimeout(500);
}

test.afterEach(() => {
    expect(errors, errors.join("\n\n")).toEqual([]);
});

test("main window", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await expect(page.locator(".react-flow__node").first()).toBeVisible();
    await settle(page);
});

test("toolbar adds a node and fades what can't run", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await expect(page.locator(".react-flow__node").first()).toBeVisible();

    // at the root graph there's no group to leave
    await expect(page.locator('.toolbar [data-command="area.exit_group"]')).toHaveClass(/disabled/);
    await expect(page.locator('.toolbar [data-command="area.exit_to_root"]')).toHaveClass(/disabled/);
    await expect(page.locator('.toolbar [data-command="area.frame_all"]')).not.toHaveClass(/disabled/);

    // typing in the add menu searches every node, enter adds the first under the cursor
    await page.locator('.toolbar [data-tool="add_node"]').click();
    await page.keyboard.type("scene link");
    await page.keyboard.press("Enter");
    const added = () =>
        page.evaluate(() =>
            (window as any).__smokeCalls
                .filter((c: any) => c.cmd === "graph_apply")
                .flatMap((c: any) => c.args.ops)
                .filter((op: any) => op.op === "add_nodes")
                .map((op: any) => op.nodes[0].type)
        );
    await expect.poll(added).toContain("scene_link");
    // the grab it starts is cancelled
    await page.keyboard.press("Escape");
    await settle(page);
});

test("main window with every panel docked", async ({ page }) => {
    await openWindow(page, withLayout(ALL_DOCKED), "/#/");
    await expect(page.locator(".dock-column .nodes-grid .node.preview").first()).toBeVisible();
    await expect(page.getByText("Move", { exact: true })).toBeVisible();
    await settle(page);
});

test("resizing the docked panels", async ({ page }) => {
    await openWindow(page, withLayout({ ...ALL_DOCKED, panelSides: { 2: "left" } }), "/#/");
    for (const [side, dx] of [
        ["left", 100],
        ["right", -100],
    ] as const) {
        const column = page.locator(`.dock-column.dock-${side}`);
        await expect(column).toBeVisible();
        const before = (await column.boundingBox())!.width;
        const handle = (await column.locator(".dock-resizer").boundingBox())!;
        await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
        await page.mouse.down();
        await page.mouse.move(handle.x + handle.width / 2 + dx, handle.y + handle.height / 2, { steps: 5 });
        await page.mouse.up();
        await expect.poll(async () => (await column.boundingBox())!.width).toBe(before + 100);
    }
    // dragged past the narrowest width, the side hides
    const left = page.locator(".dock-column.dock-left");
    const handle = (await left.locator(".dock-resizer").boundingBox())!;
    await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
    await page.mouse.down();
    await page.mouse.move(handle.x - 400, handle.y + handle.height / 2, { steps: 10 });
    await page.mouse.up();
    await expect(left).toBeHidden();
    await settle(page);
});

test("nodes panel list view", async ({ page }) => {
    await openWindow(page, withLayout(ALL_DOCKED), "/#/");
    await expect(page.locator(".nodes-grid .node.preview").first()).toBeVisible();
    await page.getByRole("button", { name: "List" }).click();
    await expect(page.locator(".nodes-list-row").first()).toBeVisible();
    await expect(page.locator(".nodes-grid")).toHaveCount(0);
    await page.getByRole("button", { name: "Grid" }).click();
    await expect(page.locator(".nodes-grid .node.preview").first()).toBeVisible();
    await settle(page);
});

test("selecting each node", async ({ page }) => {
    await openWindow(page, withLayout(ALL_DOCKED), "/#/");
    const nodes = page.locator(".react-flow__node");
    await expect(nodes.first()).toBeVisible();
    for (let i = 0; i < (await nodes.count()); i++) {
        await nodes
            .nth(i)
            .locator(".node-header, .node")
            .first()
            .click({ position: { x: 10, y: 6 }, force: true });
    }
    await settle(page);
});

test("backend events", async ({ page }) => {
    await openWindow(page, withLayout(ALL_DOCKED), "/#/");
    await expect(page.locator(".react-flow__node").first()).toBeVisible();
    const state = backend.get_state;
    await emitEvent(page, "update_state", { ...state, state_rev: state.state_rev + 1 });
    await emitEvent(page, "graph_changed", { tab: state.active_tab, graph_rev: state.graph_rev + 1, rf_instance: state.rf_instance });
    await emitEvent(page, "history_changed", { ...backend.get_history, current: 0 });
    await emitEvent(page, "project_status", backend.get_project_status);
    await emitEvent(page, "settings_changed", backend.get_settings);
    await emitEvent(page, "keymap_changed", backend.get_keymap);
    await settle(page);
});

for (const [id, name] of [
    [0, "Nodes"],
    [1, "Properties"],
    [2, "History"],
] as const) {
    test(`floating ${name} panel window`, async ({ page }) => {
        await openWindow(page, backend, `/#/panel/${id}`, `panel-${id}`);
        await expect(page.locator(".panel-header")).toContainText(name);
        await settle(page);
    });
}

test("settings window, every section", async ({ page }) => {
    await openWindow(page, backend, "/#/settings", "settings");
    const sections = page.locator("button.block");
    await expect(sections.first()).toBeVisible();
    for (let i = 0; i < (await sections.count()); i++) await sections.nth(i).click();
    await settle(page);
});

test("graph window with every node selected", async ({ page }) => {
    const state = backend.get_state;
    const nodes = state.rf_instance.nodes.map((node: any) => ({ ...node, selected: true }));
    await openWindow(page, { ...backend, get_state: { ...state, rf_instance: { ...state.rf_instance, nodes } } }, "/#/graph", "Graph");
    await settle(page);
});

test("graph window previews an imported shape", async ({ page }) => {
    const state = backend.get_state;
    const nodes = state.rf_instance.nodes.map((node: any) => ({ ...node, selected: node.id === "evaluate_instrument-1" }));
    const settings = { ...backend.get_settings, graph: { ...backend.get_settings.graph, preview_shape: "cymbal" } };
    await openWindow(page, { ...backend, get_settings: settings, get_state: { ...state, rf_instance: { ...state.rf_instance, nodes } } }, "/#/graph", "Graph");
    await expect(page.locator("select")).toHaveValue("cymbal");
    await settle(page);
});

test("graph window scrubs and plays", async ({ page }) => {
    const state = backend.get_state;
    const nodes = state.rf_instance.nodes.map((node: any) => ({ ...node, selected: node.id === "evaluate_instrument-1" }));
    await openWindow(page, { ...backend, get_state: { ...state, rf_instance: { ...state.rf_instance, nodes } } }, "/#/graph", "Graph");
    await settle(page);
    // dragging along the scrub strip on top of the curves and through the curves, then playing from there
    const box = (await page.locator("canvas").first().boundingBox())!;
    for (const y of [box.y + 10, box.y + box.height / 2]) {
        await page.mouse.move(box.x + box.width * 0.3, y);
        await page.mouse.down();
        await page.mouse.move(box.x + box.width * 0.4, y, { steps: 5 });
        await page.mouse.up();
    }
    await page.keyboard.press("Space");
    await settle(page);
    // stepping and jumping while it plays, then stopped
    for (const key of ["ArrowLeft", "ArrowRight", "Shift+ArrowLeft", "Shift+ArrowRight"]) await page.keyboard.press(key);
    await page.keyboard.press("Space");
});

test("profiler shows each node's time and the toolbar toggles it", async ({ page }) => {
    // thresholds low enough that the fixture's group is slow and its quick nodes medium on any machine
    const settings = { ...backend.get_settings, console: { ...backend.get_settings.console, profiler: true, profiler_medium_ms: 0.05, profiler_slow_ms: 1 } };
    await openWindow(page, { ...backend, get_settings: settings }, "/#/");
    await expect(page.locator(".react-flow__node .node-time").first()).toBeVisible();
    await expect(page.locator(".node-time.console-error").first()).toBeVisible();
    await expect(page.locator(".node-time.console-warn").first()).toBeVisible();
    await expect(page.locator('.toolbar [data-command="profiler"]')).toHaveClass(/active/);
    await page.locator('.toolbar [data-command="profiler"]').click();
    const toggled = () => page.evaluate(() => (window as any).__smokeCalls.some((c: any) => c.cmd === "run_app_command" && c.args.id === "profiler"));
    await expect.poll(toggled).toBe(true);
    // turned off from the menu, the times go
    await emitEvent(page, "settings_changed", backend.get_settings);
    await expect(page.locator(".node-time")).toHaveCount(0);
    await settle(page);
});

test("console window filters, finds and shows the run", async ({ page }) => {
    await openWindow(page, backend, "/#/console", "Console");
    // the fixture's run is in the backlog
    await expect(page.getByText("Run finished", { exact: false }).first()).toBeVisible();
    // each filter off and on again, the source "Run" comes after the view's
    for (const name of ["Error", "Warn", "Info", "Debug", "Run", "Node", "Blender", "App", "Rust", "JS"]) {
        await page.getByRole("button", { name, exact: true }).last().click();
        await page.getByRole("button", { name, exact: true }).last().click();
    }
    // find opens on its key, enter steps through the matches, escape closes it
    await page.keyboard.press("ControlOrMeta+f");
    await page.keyboard.type("run");
    await expect(page.locator("mark.console-match").first()).toBeVisible();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Shift+Enter");
    await page.keyboard.press("Escape");
    await expect(page.locator("mark.console-match")).toHaveCount(0);
    // rows pick and copy
    await page.getByText("Run finished", { exact: false }).first().click();
    await page.getByRole("button", { name: "Copy", exact: true }).click();
    // the run's timings, sorted by each column both ways
    await page.getByRole("button", { name: "Run", exact: true }).first().click();
    await expect(page.locator("tbody tr").first()).toBeVisible();
    for (const header of await page.locator("th").all()) {
        await header.click();
        await header.click();
    }
    await emitEvent(page, "run_timings", backend.get_state.active_tab);
    await emitEvent(page, "console_entries", [{ id: 1e9, time: Date.now(), level: "error", source: "node", message: "failed", node: "viewer-1", tab: "tab-1", ms: 1 }]);
    await emitEvent(page, "console_cleared", null);
    await settle(page);
});

// the fixture with only `id` selected and every panel docked
function withSelected(id: string) {
    const state = backend.get_state;
    const nodes = state.rf_instance.nodes.map((node: any) => ({ ...node, selected: node.id === id }));
    return { ...backend, get_state: { ...state, layout: ALL_DOCKED, rf_instance: { ...state.rf_instance, nodes } } };
}

// a row of the properties panel by its name, and its value
const propertiesRow = (page: Page, name: string) => page.locator(".properties-row").filter({ has: page.locator(".properties-name", { hasText: new RegExp(`^${name.replace(/\./g, "\\.")}$`) }) });

test("properties panel shows outputs from the last run", async ({ page }) => {
    await openWindow(page, withSelected("get_midi_track_data-1"), "/#/");
    await expect(propertiesRow(page, "Unique Note Numbers")).toContainText("60, 61, 62");
    await settle(page);
});

test("properties panel shows the notes each object got", async ({ page }) => {
    await openWindow(page, withSelected("assign_notes_to_objects-1"), "/#/");
    // the fixture's object map, note names like the old add-on (60 = C3)
    await expect(propertiesRow(page, "ANIM_bounce")).toContainText("59/B2");
    await expect(propertiesRow(page, "Cube.001")).toContainText("60/C3");
    await expect(propertiesRow(page, "Cube.005")).toContainText("64/E3");
    await settle(page);
});

test("note numbers field keeps what's typed", async ({ page }) => {
    await openWindow(page, withLayout(ALL_DOCKED), "/#/");
    // the node's only text box, under Note Numbers (Object Group Name and Mode are dropdowns)
    const field = page.locator(".react-flow__node", { hasText: "Assign Notes to Objects" }).locator('input[type="text"]');
    await field.fill("[60 61, x]");
    await field.evaluate((input: HTMLInputElement) => input.blur());

    // set as it was typed, the node reads it when it runs
    const sets = await page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").flatMap((c: any) => c.args.ops));
    expect(sets).toContainEqual({ op: "set_inputs", node: "assign_notes_to_objects-1", inputs: { note_numbers: "[60 61, x]" } });
    await settle(page);
});

test("note map opens over the graph and closes", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await page.locator(".react-flow__node", { hasText: "Assign Notes to Objects" }).locator(".group-open").click();

    // the fixture's 6 objects in the Cubes group, and its notes 60-62 padded out with notes the MIDI doesn't play
    const map = page.locator(".note-map-layer");
    await expect(map).toBeVisible();
    await expect(map.locator(".react-flow__node", { hasText: "Cube.001" })).toBeVisible();
    await expect(map.locator('.react-flow__node[data-id^="o:"]')).toHaveCount(6);
    await expect(map.locator('.react-flow__node[data-id="n:59"] .note-map-added')).toBeVisible();
    await expect(map.locator('.react-flow__node[data-id="n:60"] .note-map-added')).toHaveCount(0);

    // shift+a takes a note number or name, enter adds it under the cursor and switches the node to map mode
    const addedNotes = async () => {
        const ops = await page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").flatMap((c: any) => c.args.ops));
        return ops.filter((op: any) => op.op === "set_inputs" && op.inputs.note_map);
    };
    const box = page.locator(".note-add-box input");
    await page.mouse.move(640, 600);
    for (const [typed, note] of [
        ["70", 70],
        ["c#4", 73],
    ] as const) {
        await page.keyboard.press("Shift+A");
        await expect(box).toBeFocused();
        await page.keyboard.type(typed);
        await page.keyboard.press("Enter");
        await expect(box).toHaveCount(0);
        const added = (await addedNotes()).at(-1);
        expect(added.inputs.mode).toBe("map");
        expect(added.inputs.note_map.notes).toContain(note);
        expect(added.inputs.map_layout[`n:${note}`]).toBeTruthy();
    }

    // a click outside the box closes it without adding anything
    const count = (await addedNotes()).length;
    await page.keyboard.press("Shift+A");
    await expect(box).toBeVisible();
    await page.mouse.click(700, 650);
    await expect(box).toHaveCount(0);
    expect((await addedNotes()).length).toBe(count);
    await page.keyboard.press("Tab");
    await expect(map).toHaveCount(0);
    await settle(page);
});

test("a reload keeps the open note map and collapsed sections", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await page.locator(".react-flow__node", { hasText: "Assign Notes to Objects" }).locator(".group-open").click();
    await expect(page.locator(".note-map-layer")).toBeVisible();

    // the nodes panel's MIDI section collapsed
    const grids = page.locator(".nodes-panel .nodes-grid");
    const sections = await grids.count();
    await page
        .locator(".nodes-panel .properties-section")
        .filter({ has: page.getByText("MIDI", { exact: true }) })
        .click();
    await expect(grids).toHaveCount(sections - 1);

    // the page is loaded again, the backend is the same
    await page.reload();
    await expect(page.locator(".note-map-layer")).toBeVisible();
    await expect(grids).toHaveCount(sections - 1);
    await settle(page);
});

test("note map keeps its nodes when a run sends new results", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await page.locator(".react-flow__node", { hasText: "Assign Notes to Objects" }).locator(".group-open").click();
    const map = page.locator(".note-map-layer");
    await expect(map.locator(".react-flow__node")).toHaveCount(12);

    // realtime runs one after another send the results again as new objects, the same or changed. react flow hides a
    // node rebuilt without its measured size until it measures it again, quick rebuilds left them all hidden
    await page.evaluate(async (state) => {
        for (let i = 1; i <= 40; i++) {
            const next = structuredClone(state);
            const entry = next.executed_results["assign_notes_to_objects-1"].object_map.objects["Cube.005"];
            if (i % 4 === 0) for (const k of Object.keys(entry)) entry[k] = [70];
            (window as any).__smokeEmit("update_state", { ...next, state_rev: (state.state_rev ?? 0) + i });
            await new Promise((r) => setTimeout(r, i % 3 === 0 ? 0 : 16));
        }
    }, backend.get_state);
    await page.waitForTimeout(300);
    for (const node of await map.locator(".react-flow__node").all()) await expect(node).toBeVisible();
    await settle(page);
});

// the graph on screen is ready: until then it's drawn see-through and takes no clicks (NodeGraph.tsx). a locator's click
// waits for that, a click at a point with the mouse doesn't, and opacity 0 still counts as visible
async function graphReady(page: Page) {
    await expect(page.locator('.graph-layer:not([style*="opacity"]) .react-flow__node').first()).toBeAttached();
}

// selects the nodes `ids` and frames them (Frame Selected), so a test that goes by screen distances sees them at about
// full size. the whole fixture is framed small, its viewer is tall and the docked panels take a side each
async function frameNodes(page: Page, ids: string[]) {
    for (const [i, id] of ids.entries()) {
        await page.locator(`.react-flow__node[data-id="${id}"] .node-header`).click({ modifiers: i > 0 ? ["Shift"] : [] });
    }
    const pane = (await page.locator(".node-graph").boundingBox())!;
    await page.mouse.move(pane.x + pane.width / 2, pane.y + pane.height - 40);
    const viewport = page.locator(".graph-layer .react-flow__viewport").first();
    const before = await viewport.getAttribute("style");
    await page.keyboard.press(".");
    await expect(viewport).not.toHaveAttribute("style", before!);
}

// a point on an edge's path on screen where it's the edge a click gets, the nearest to its middle. another edge or a
// node can be over the middle depending on how the nodes measure (fonts differ between platforms). on whole pixels, a
// right click's context menu is found at the pixel the mouse is on
async function edgePoint(edge: ReturnType<Page["locator"]>) {
    return edge.evaluate((g: Element) => {
        const path = g.querySelector<SVGPathElement>("path.react-flow__edge-path")!;
        const ctm = path.getScreenCTM()!;
        const at = (fraction: number) => {
            const point = path.getPointAtLength(path.getTotalLength() * fraction);
            return { x: Math.round(point.x * ctm.a + ctm.e), y: Math.round(point.y * ctm.d + ctm.f) };
        };
        for (let step = 0; step <= 8; step++) {
            for (const fraction of [0.5 - step * 0.05, 0.5 + step * 0.05]) {
                const point = at(fraction);
                if (document.elementFromPoint(point.x, point.y)?.closest(".react-flow__edge") === g) return point;
            }
        }
        return at(0.5);
    });
}

// the middle of an edge's path on screen
async function edgeMiddle(edge: ReturnType<Page["locator"]>) {
    return edge.locator("path.react-flow__edge-path").evaluate((path: SVGPathElement) => {
        const point = path.getPointAtLength(path.getTotalLength() / 2);
        const ctm = path.getScreenCTM()!;
        return { x: point.x * ctm.a + ctm.e, y: point.y * ctm.d + ctm.f };
    });
}

test("clicking an edge selects it in every graph", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    const edge = page.locator('.react-flow__edge[data-id="xy-edge__assign_notes_to_objects-1midi_notes-get_midi_track_data-1notes"]');
    await expect(edge).toBeVisible();
    await graphReady(page);
    await settle(page);
    const point = await edgePoint(edge);
    await page.mouse.click(point.x, point.y);
    await expect(edge).toHaveClass(/selected/);
    await expect(edge.locator(".edge-ring")).toHaveCount(2);
    const selects = await page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").flatMap((c: any) => c.args.ops));
    expect(selects.at(-1)).toMatchObject({ op: "select", nodes: [], edges: [await edge.getAttribute("data-id")] });

    // and in the note map
    await page.locator(".react-flow__node", { hasText: "Assign Notes to Objects" }).locator(".group-open").click();
    await expect(page.locator(".note-map-layer .react-flow__node")).toHaveCount(12);
    const wire = page.locator(".note-map-layer .react-flow__edge").nth(2);
    const wirePoint = await edgePoint(wire);
    await page.mouse.click(wirePoint.x, wirePoint.y);
    await expect(wire).toHaveClass(/selected/);
    await expect(wire.locator(".edge-ring")).toHaveCount(2);
    await settle(page);
});

test("box select takes the edges it crosses", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    const edge = page.locator('.react-flow__edge[data-id="xy-edge__animation_generator-1note_on_keyframes-keyframes_from_object-1location[2]"]');
    await expect(edge).toBeVisible();
    await graphReady(page);
    await settle(page);
    await frameNodes(page, ["animation_generator-1", "keyframes_from_object-1"]);

    // a small box from empty space across the middle of the wire, no node or other wire in it
    const middle = await edgeMiddle(edge);
    const start = { x: middle.x - 20, y: middle.y - 20 };
    expect(await page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.classList.contains("react-flow__pane"), start)).toBe(true);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(middle.x + 20, middle.y + 20, { steps: 8 });
    await page.mouse.up();
    await expect(edge).toHaveClass(/selected/);
    await expect(page.locator(".react-flow__edge.selected")).toHaveCount(1);
    await expect(page.locator(".react-flow__node.selected")).toHaveCount(0);

    // and goes to the backend like any selection
    await expect
        .poll(async () => {
            const ops = await page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").flatMap((c: any) => c.args.ops));
            return ops.filter((op: any) => op.op === "select").at(-1);
        })
        .toMatchObject({ op: "select", nodes: [], edges: ["xy-edge__animation_generator-1note_on_keyframes-keyframes_from_object-1location[2]"] });
    await settle(page);
});

test("note map box select takes the wires it crosses", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await page.locator(".react-flow__node", { hasText: "Assign Notes to Objects" }).locator(".group-open").click();
    const map = page.locator(".note-map-layer");
    await expect(map.locator(".react-flow__node")).toHaveCount(12);

    // a box from empty space above the wires down across the first three, between the notes and the objects
    const note = (await map.locator('.react-flow__node[data-id="n:61"]').boundingBox())!;
    const object = (await map.locator('.react-flow__node[data-id="o:Cube.002"]').boundingBox())!;
    const x = (note.x + note.width + object.x) / 2;
    const top = (await map.locator('.react-flow__node[data-id="n:59"]').boundingBox())!.y - 60;
    await page.mouse.move(x - 30, top);
    await page.mouse.down();
    const socket = (await map.locator('.react-flow__node[data-id="n:61"] .react-flow__handle').boundingBox())!;
    await page.mouse.move(x + 30, socket.y + socket.height / 2 + 5, { steps: 8 });
    await page.mouse.up();
    await expect(map.locator(".react-flow__edge.selected")).toHaveCount(3);
    await expect(map.locator(".react-flow__node.selected")).toHaveCount(0);

    // x removes those wires only, switching the node to map mode
    await page.keyboard.press("x");
    const sets = await page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").flatMap((c: any) => c.args.ops));
    const removed = sets.find((op: any) => op.op === "set_inputs" && op.inputs.note_map);
    expect(removed.inputs.note_map.objects).toEqual({ "Cube.003": [62], "Cube.004": [63], "Cube.005": [64] });
    await settle(page);
});

test("map mode hides the note numbers and previews the map", async ({ page }) => {
    // the fixture with Assign Notes to Objects in map mode
    const state = structuredClone(backend.get_state);
    const assign = state.rf_instance.nodes.find((n: any) => n.id === "assign_notes_to_objects-1");
    assign.data.inputs = { ...assign.data.inputs, mode: "map", note_map: { objects: { "Cube.001": [60, 61], "Cube.003": [61], "Cube.005": [62, 70] }, notes: [70] } };
    await openWindow(page, { ...backend, get_state: state }, "/#/");

    const node = page.locator(".react-flow__node", { hasText: "Assign Notes to Objects" });
    await expect(node.locator("select").nth(1)).toHaveValue("map");
    await expect(node.getByText("Notes", { exact: true })).toBeVisible();
    await expect(node.getByText("Note Numbers", { exact: true })).toBeHidden();
    await expect(node.locator('input[type="text"]')).toHaveCount(0);

    // the preview shows the map: a line per note an object gets, the MIDI's 3 notes and the added one, every object
    const preview = node.locator(".node-field svg");
    await expect(preview.locator("path")).toHaveCount(5);
    await expect(preview.locator(`rect[fill="#B8962E"]`)).toHaveCount(4);
    await expect(preview.locator(`rect[fill="#3E9E72"]`)).toHaveCount(6);
    await settle(page);
});

// the fixture with signal tags: get_midi_track_data-1's notes feed both their inputs by the tag "notes" instead of
// wires, and "unused" on its pitchwheel output connects nothing
function withTags() {
    const state = structuredClone(backend.get_state);
    const rf = state.rf_instance;
    const node = (id: string) => rf.nodes.find((n: any) => n.id === id);
    node("get_midi_track_data-1").data.output_tags = { notes: "notes", pitchwheel: "unused" };
    for (const id of ["assign_notes_to_objects-1", "evaluate_instrument-1"]) node(id).data.input_tags = { midi_notes: "notes" };
    for (const edge of rf.edges) if (edge.target === "get_midi_track_data-1" && edge.targetHandle === "notes") edge.tagged = true;
    return { ...backend, get_state: state };
}

test("signal tags", async ({ page }) => {
    await openWindow(page, withTags(), "/#/");
    const source = page.locator('.react-flow__node[data-id="get_midi_track_data-1"]');
    await expect(source.locator(".socket-tag")).toHaveCount(2);
    await expect(source.locator(".socket-tag.broken")).toHaveText("unused");
    // each input shows the tag it takes its value from, its wire isn't drawn
    for (const id of ["assign_notes_to_objects-1", "evaluate_instrument-1"]) {
        const tag = page.locator(`.react-flow__node[data-id="${id}"] .socket-tag-inputs`);
        await expect(tag).toHaveText("notes");
        await expect(tag).not.toHaveClass(/broken/);
    }
    await expect(page.locator('.react-flow__edge[data-id*="get_midi_track_data-1notes"]')).toHaveCount(0);

    // hovering a tag lights up every tag with its name
    await source.locator(".socket-tag-outputs", { hasText: "notes" }).hover();
    await expect(page.locator(".socket-tag.lit")).toHaveCount(3);

    // double clicking a socket opens the tag menu like shift+a: it searches the outputs' tags, enter picks the first
    const tagOps = async () => {
        const ops = await page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").flatMap((c: any) => c.args.ops));
        return ops.filter((op: any) => op.op === "set_tag");
    };
    const viewerInput = page.locator('.react-flow__node[data-id="viewer-1"] .react-flow__handle[data-handleid="data"]');
    const menu = page.locator(".tag-menu");
    await viewerInput.dblclick();
    await expect(menu.locator("input")).toBeFocused();
    await expect(menu.locator(".tag-menu-row")).toHaveText(["notes", "unused"]);
    await expect(menu.locator("input")).toHaveAttribute("placeholder", "Add tag");
    await page.keyboard.type("no");
    await expect(menu.locator(".tag-menu-row")).toHaveText(["notes", "no"]);
    await page.keyboard.press("Enter");
    await expect(menu).toHaveCount(0);
    expect((await tagOps()).at(-1)).toEqual({ op: "set_tag", node: "viewer-1", side: "inputs", socket: "data", name: "notes" });

    // a name no output has comes first when nothing matches it
    await viewerInput.dblclick();
    await page.keyboard.type("audio 1");
    await expect(menu.locator(".tag-menu-row")).toHaveText(["audio 1"]);
    await page.keyboard.press("Enter");
    expect((await tagOps()).at(-1)).toEqual({ op: "set_tag", node: "viewer-1", side: "inputs", socket: "data", name: "audio 1" });

    // escape closes it without tagging anything, an output gets the broken tags
    await page.locator('.react-flow__node[data-id="get_midi_track_data-1"] .react-flow__handle[data-handleid="notes"]').dblclick();
    await expect(menu.locator("input")).toBeFocused();
    await expect(menu.locator("input")).toHaveAttribute("placeholder", "Rename tag");
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);
    expect((await tagOps()).length).toBe(2);

    // a click on a tag selects its socket on its own, not its node, and delete removes the tag like an edge
    const inputTag = page.locator('.react-flow__node[data-id="evaluate_instrument-1"] .socket-tag-inputs');
    await inputTag.click();
    await expect(inputTag).toHaveClass(/selected/);
    await expect(page.locator(".react-flow__node.selected")).toHaveCount(0);
    const selects = await page.evaluate(() =>
        (window as any).__smokeCalls
            .filter((c: any) => c.cmd === "graph_apply")
            .flatMap((c: any) => c.args.ops)
            .filter((op: any) => op.op === "select")
    );
    expect(selects.at(-1)).toEqual({ op: "select", nodes: [], edges: [], sockets: [{ node: "evaluate_instrument-1", side: "inputs", socket: "midi_notes" }] });
    await page.keyboard.press("x");
    const deletes = await page.evaluate(() =>
        (window as any).__smokeCalls
            .filter((c: any) => c.cmd === "graph_apply")
            .flatMap((c: any) => c.args.ops)
            .filter((op: any) => op.op === "delete")
    );
    expect(deletes.at(-1)).toEqual({ op: "delete", nodes: [], edges: [], sockets: [{ node: "evaluate_instrument-1", side: "inputs", socket: "midi_notes" }] });

    // clicking the empty graph deselects it
    await page.mouse.click(700, 700);
    await expect(inputTag).not.toHaveClass(/selected/);
    await settle(page);
});

// the graph_apply calls sent so far, each one's ops
async function applied(page: Page): Promise<any[][]> {
    return page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").map((c: any) => c.args.ops));
}

const socketOf = (page: Page, node: string, socket: string) => page.locator(`.react-flow__node[data-id="${node}"] .react-flow__handle[data-handleid="${socket}"]`);

test("selecting sockets and dragging links off several at once", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    const notes = socketOf(page, "get_midi_track_data-1", "notes");
    const numbers = socketOf(page, "get_midi_track_data-1", "unique_note_numbers");
    await expect(notes).toBeVisible();
    await graphReady(page);
    await settle(page);

    // a click selects a socket on its own, not its node, shift adds another
    await notes.click();
    await expect(notes).toHaveClass(/socket-selected/);
    await expect(page.locator(".react-flow__node.selected")).toHaveCount(0);
    await numbers.click({ modifiers: ["Shift"] });
    await expect(numbers).toHaveClass(/socket-selected/);
    await expect(notes).toHaveClass(/socket-selected/);
    await expect
        .poll(async () =>
            (await applied(page))
                .flat()
                .filter((op) => op.op === "select")
                .at(-1)
        )
        .toEqual({
            op: "select",
            nodes: [],
            edges: [],
            sockets: [
                { node: "get_midi_track_data-1", side: "outputs", socket: "notes" },
                { node: "get_midi_track_data-1", side: "outputs", socket: "unique_note_numbers" },
            ],
        });

    // a link dragged off one brings the other, drawn along with it. dropped on an input, the other connects to the free
    // input of its type below it, in the same edit
    const from = (await notes.boundingBox())!;
    const to = (await socketOf(page, "assign_notes_to_objects-1", "midi_notes").boundingBox())!;
    const calls = (await applied(page)).length;
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(to.x - 40, to.y, { steps: 6 });
    await expect(page.locator(".react-flow__connection path.node-edge")).toHaveCount(2);
    await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 6 });
    // over the input, the other one is drawn going into the input it will connect to
    const target = (await socketOf(page, "assign_notes_to_objects-1", "note_numbers").boundingBox())!;
    const ends = await page.locator(".react-flow__connection path.node-edge").evaluateAll((paths: SVGPathElement[]) =>
        paths.map((path) => {
            const end = path.getPointAtLength(path.getTotalLength());
            const ctm = path.getScreenCTM()!;
            return { x: end.x * ctm.a + ctm.e, y: end.y * ctm.d + ctm.f };
        })
    );
    expect(ends.some((end) => Math.abs(end.x - (target.x + target.width / 2)) < 3 && Math.abs(end.y - (target.y + target.height / 2)) < 3)).toBe(true);
    await page.mouse.up();
    await expect
        .poll(async () => (await applied(page)).slice(calls).filter((ops) => ops.some((op) => op.op === "connect")))
        .toEqual([
            [
                { op: "connect", from_node: "get_midi_track_data-1", from_output: "notes", to_node: "assign_notes_to_objects-1", to_input: "midi_notes" },
                { op: "connect", from_node: "get_midi_track_data-1", from_output: "unique_note_numbers", to_node: "assign_notes_to_objects-1", to_input: "note_numbers" },
            ],
        ]);

    // a node with no free socket of the right type left, the next nearest one takes it
    const evaluate = (await socketOf(page, "evaluate_instrument-1", "midi_notes").boundingBox())!;
    const before = (await applied(page)).length;
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(evaluate.x - 40, evaluate.y, { steps: 6 });
    await page.mouse.move(evaluate.x + evaluate.width / 2, evaluate.y + evaluate.height / 2, { steps: 6 });
    await page.mouse.up();
    await expect
        .poll(async () => (await applied(page)).slice(before).filter((ops) => ops.some((op) => op.op === "connect")))
        .toEqual([
            [
                { op: "connect", from_node: "get_midi_track_data-1", from_output: "notes", to_node: "evaluate_instrument-1", to_input: "midi_notes" },
                { op: "connect", from_node: "get_midi_track_data-1", from_output: "unique_note_numbers", to_node: "assign_notes_to_objects-1", to_input: "note_numbers" },
            ],
        ]);

    // they fill in order whatever their type, like the dragged link: control change takes the next free input
    await numbers.click({ modifiers: ["Shift"] });
    await socketOf(page, "get_midi_track_data-1", "control_change").click({ modifiers: ["Shift"] });
    const last = (await applied(page)).length;
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(to.x - 40, to.y, { steps: 6 });
    await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 6 });
    await page.mouse.up();
    await expect
        .poll(async () => (await applied(page)).slice(last).filter((ops) => ops.some((op) => op.op === "connect")))
        .toEqual([
            [
                { op: "connect", from_node: "get_midi_track_data-1", from_output: "notes", to_node: "assign_notes_to_objects-1", to_input: "midi_notes" },
                { op: "connect", from_node: "get_midi_track_data-1", from_output: "control_change", to_node: "assign_notes_to_objects-1", to_input: "note_numbers" },
            ],
        ]);

    // dropped on nothing they don't open the add menu, and there's no + for it
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(from.x + 120, from.y + 200, { steps: 6 });
    await page.waitForTimeout(50);
    await expect(page.locator(".react-flow__connection .edge-plus-sign")).toHaveCount(0);
    await page.mouse.up();
    await page.waitForTimeout(200);
    await expect(page.getByPlaceholder("Search nodes...")).toHaveCount(0);
    await settle(page);
});

test("dragging off a connected input picks up its link", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    const notes = socketOf(page, "assign_notes_to_objects-1", "midi_notes");
    await expect(notes).toBeVisible();
    await graphReady(page);
    await settle(page);
    const link = "xy-edge__assign_notes_to_objects-1midi_notes-get_midi_track_data-1notes";
    const box = (await notes.boundingBox())!;
    const [x, y] = [box.x + box.width / 2, box.y + box.height / 2];
    // the edits a drag made, without the selection
    const edits = async (drag: () => Promise<void>) => {
        const before = (await applied(page)).length;
        await drag();
        await page.waitForTimeout(300);
        return (await applied(page)).slice(before).filter((ops) => ops.some((op) => op.op !== "select"));
    };

    // a click leaves it
    expect(await edits(() => notes.click())).toEqual([]);

    // dropped on nothing it's removed, hidden while it's dragged and no add menu opens
    const removed = await edits(async () => {
        await page.mouse.move(x, y);
        await page.mouse.down();
        await page.mouse.move(x - 150, y + 120, { steps: 8 });
        await expect(page.locator(`.react-flow__edge[data-id="${link}"]`)).toHaveCount(0);
        await page.mouse.up();
    });
    expect(removed).toEqual([[{ op: "delete", edges: [link] }]]);
    await expect(page.getByPlaceholder("Search nodes...")).toHaveCount(0);

    // dropped on another input it moves there in one edit
    const to = (await socketOf(page, "evaluate_instrument-1", "midi_notes").boundingBox())!;
    const moved = await edits(async () => {
        await page.mouse.move(x, y);
        await page.mouse.down();
        await page.mouse.move(to.x - 30, to.y, { steps: 6 });
        await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 6 });
        await page.mouse.up();
    });
    expect(moved).toEqual([
        [
            { op: "delete", edges: [link] },
            { op: "connect", from_node: "get_midi_track_data-1", from_output: "notes", to_node: "evaluate_instrument-1", to_input: "midi_notes" },
        ],
    ]);
    await settle(page);
});

test("a multi input takes a link dropped anywhere along it", async ({ page }) => {
    // five links into the generator's note on keyframes make its socket tall
    const tall = JSON.parse(JSON.stringify(backend));
    for (const output of ["control_change", "pitchwheel", "aftertouch", "unique_note_numbers"]) {
        tall.get_state.rf_instance.edges.push({ id: `extra-${output}`, source: "animation_generator-1", sourceHandle: "note_on_keyframes", target: "get_midi_track_data-1", targetHandle: output });
    }
    await openWindow(page, tall, "/#/");
    const socket = socketOf(page, "animation_generator-1", "note_on_keyframes");
    await expect(socket).toBeVisible();
    await graphReady(page);
    await settle(page);
    await frameNodes(page, ["get_midi_track_data-1", "animation_generator-1"]);
    const box = (await socket.boundingBox())!;
    const from = (await socketOf(page, "get_midi_track_data-1", "notes").boundingBox())!;

    // beside its ends and just above it, further than react flow looks around a socket's middle
    for (const [x, y] of [
        [box.x - 10, box.y + 2],
        [box.x - 10, box.y + box.height - 2],
        [box.x + box.width / 2, box.y - 6],
    ]) {
        const before = (await applied(page)).length;
        await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
        await page.mouse.down();
        await page.mouse.move(x - 40, y, { steps: 6 });
        await page.mouse.move(x, y, { steps: 6 });
        await expect(page.locator(".react-flow__connection")).toHaveClass(/valid/);
        await page.mouse.up();
        await expect.poll(async () => (await applied(page)).slice(before).filter((ops) => ops.some((op) => op.op === "connect"))).toEqual([[{ op: "connect", from_node: "get_midi_track_data-1", from_output: "notes", to_node: "animation_generator-1", to_input: "note_on_keyframes" }]]);
    }
    await settle(page);
});

test("tagging several sockets at once", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    const first = socketOf(page, "evaluate_instrument-1", "midi_notes");
    const second = socketOf(page, "assign_notes_to_objects-1", "midi_notes");
    await expect(first).toBeVisible();
    await settle(page);
    await first.click();
    await second.click({ modifiers: ["Shift"] });

    // double clicking one of them tags them all, the selection stays
    await second.dblclick();
    const menu = page.locator(".tag-menu");
    await expect(menu.locator("input")).toBeFocused();
    await page.keyboard.type("midi");
    await page.keyboard.press("Enter");
    const setTags = (await applied(page)).flat().filter((op) => op.op === "set_tags");
    expect(setTags).toHaveLength(1);
    expect(setTags[0].name).toBe("midi");
    expect(setTags[0].sockets).toHaveLength(2);
    expect(setTags[0].sockets).toEqual(
        expect.arrayContaining([
            { node: "evaluate_instrument-1", side: "inputs", socket: "midi_notes" },
            { node: "assign_notes_to_objects-1", side: "inputs", socket: "midi_notes" },
        ])
    );
    await page.waitForTimeout(700);
    await expect(first).toHaveClass(/socket-selected/);
    await expect(second).toHaveClass(/socket-selected/);

    // a single click on one of them selects it alone
    await second.click();
    await expect(first).not.toHaveClass(/socket-selected/);
    await expect(second).toHaveClass(/socket-selected/);
    await settle(page);
});

test("box select takes nodes, edges and sockets", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    const node = page.locator('.react-flow__node[data-id="get_midi_track_data-1"]');
    await expect(node).toBeVisible();
    await graphReady(page);
    await settle(page);
    await frameNodes(page, ["get_midi_track_data-1", "evaluate_instrument-1"]);

    // a narrow box down the outputs, from above the node to the aftertouch socket: the sockets in it, the node it
    // touches and that node's edges
    const bounds = (await node.boundingBox())!;
    const aftertouch = (await socketOf(page, "get_midi_track_data-1", "aftertouch").boundingBox())!;
    const right = bounds.x + bounds.width;
    await page.mouse.move(right + 20, bounds.y - 10);
    await page.mouse.down();
    await page.mouse.move(right - 3, aftertouch.y + aftertouch.height / 2 + 2, { steps: 8 });
    await page.mouse.up();
    for (const socket of ["notes", "control_change", "pitchwheel", "aftertouch"]) await expect(socketOf(page, "get_midi_track_data-1", socket)).toHaveClass(/socket-selected/);
    await expect(page.locator(".socket-selected")).toHaveCount(4);
    await expect(page.locator(".react-flow__node.selected")).toHaveCount(1);
    await expect(node).toHaveClass(/selected/);
    await expect(page.locator('.react-flow__edge.selected[data-id*="get_midi_track_data-1"]')).toHaveCount(3);

    // a box around a whole node takes the node, not its sockets
    const whole = page.locator('.react-flow__node[data-id="evaluate_instrument-1"]');
    const wholeBounds = (await whole.boundingBox())!;
    const start = { x: wholeBounds.x + wholeBounds.width + 25, y: wholeBounds.y - 20 };
    expect(await page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.classList.contains("react-flow__pane"), start)).toBe(true);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(wholeBounds.x - 25, wholeBounds.y + wholeBounds.height + 20, { steps: 8 });
    await page.mouse.up();
    await expect(whole).toHaveClass(/selected/);
    await expect(page.locator(".socket-selected")).toHaveCount(0);
    await settle(page);
});

test("status bar shows the keys that do something", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await expect(page.locator(".react-flow__node").first()).toBeVisible();
    const status = page.locator(".status-bar");
    const pane = (await page.locator(".node-graph").boundingBox())!;

    // the node editor's keys under the mouse, the ones held modifiers run while they're held
    await page.mouse.move(pane.x + pane.width / 2, pane.y + pane.height - 40);
    await expect(status).toContainText("Grab");
    await expect(status).not.toContainText("Add Node");
    await page.keyboard.down("Shift");
    await expect(status).toContainText("Add Node");
    await expect(status).not.toContainText("Grab");
    await page.keyboard.up("Shift");
    await expect(status).toContainText("Grab");

    // a modal's own keys while it's open
    await page.keyboard.press("Shift+A");
    await expect(status).toContainText("Add First");
    await page.keyboard.press("Escape");
    await expect(status).not.toContainText("Add First");
    await expect(status).toContainText("Grab");
    await settle(page);
});

test("drag ghost window", async ({ page }) => {
    await openWindow(page, backend, "/#/drag-ghost", "drag-ghost");
    await settle(page);
});

test("splash window", async ({ page }) => {
    await openWindow(page, backend, "/splash.html", "splashscreen");
    await settle(page);
});

// the ops the page has sent to graph_apply
const sentOps = (page: Page) => page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply").flatMap((c: any) => c.args.ops));

// the last op of a kind the page has sent to graph_apply
const lastOp = async (page: Page, op: string) => (await sentOps(page)).filter((o: any) => o.op === op).at(-1);

// a point of the empty graph where a click gets the pane, outside `away` (a menu open on the graph)
async function panePoint(page: Page, away?: { x: number; y: number; width: number; height: number }) {
    return page.evaluate((away) => {
        const pane = document.querySelector('.graph-layer:not([style*="opacity"]) .react-flow__pane')!.getBoundingClientRect();
        const inside = (x: number, y: number) => !!away && x >= away.x - 20 && x <= away.x + away.width + 20 && y >= away.y - 20 && y <= away.y + away.height + 20;
        for (let y = pane.top + 40; y < pane.bottom - 40; y += 20) {
            for (let x = pane.left + 40; x < pane.right - 40; x += 20) {
                if (!inside(x, y) && document.elementFromPoint(x, y)?.classList.contains("react-flow__pane")) return { x, y };
            }
        }
        throw new Error("no empty point on the graph");
    }, away);
}

test("context menu on an edge, a node and the empty graph", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await graphReady(page);
    await settle(page);
    const menu = page.locator(".context-menu");
    const rows = menu.locator(".context-menu-row");

    // an edge outside the selection is selected alone, delete disconnects it
    const id = "xy-edge__assign_notes_to_objects-1midi_notes-get_midi_track_data-1notes";
    const edge = page.locator(`.react-flow__edge[data-id="${id}"]`);
    const point = await edgePoint(edge);
    await page.mouse.click(point.x, point.y, { button: "right" });
    await expect(rows).toHaveText(["Delete"]);
    await expect(edge).toHaveClass(/selected/);
    await rows.click();
    await expect(menu).toHaveCount(0);
    expect((await sentOps(page)).at(-1)).toMatchObject({ op: "delete", nodes: [], edges: [id] });

    // a node outside the selection is selected alone, then it's the node's entries. delete runs the editor's delete
    await page.locator('.react-flow__node[data-id="get_midi_track_data-1"] .node-header').click({ button: "right" });
    await expect(rows).toHaveText(["Cut", "Copy", "Paste", "Duplicate", "Delete", "Join in New Frame", "Make Group"]);
    await expect.poll(async () => (await sentOps(page)).filter((op: any) => op.op === "select").at(-1)).toMatchObject({ nodes: ["get_midi_track_data-1"], edges: [] });
    await rows.getByText("Delete", { exact: true }).click();
    await expect(menu).toHaveCount(0);
    expect((await sentOps(page)).at(-1)).toMatchObject({ op: "delete", nodes: ["get_midi_track_data-1"], edges: [] });

    // a group node can be ungrouped and opened, escape closes the menu
    await page.locator('.react-flow__node[data-id="evaluate_instrument-1"] .node-header').click({ button: "right" });
    await expect(rows).toHaveText(["Cut", "Copy", "Paste", "Duplicate", "Delete", "Join in New Frame", "Make Group", "Ungroup", "Edit Group"]);
    await page.keyboard.press("Escape");
    await expect(menu).toHaveCount(0);

    // the empty graph, add node opens the add menu where it was right clicked
    const empty = await panePoint(page);
    await page.mouse.click(empty.x, empty.y, { button: "right" });
    await expect(rows).toHaveText(["Add Node", "Paste", "Select All", "Frame All"]);
    await rows.getByText("Add Node", { exact: true }).click();
    const search = page.getByPlaceholder("Search nodes...");
    await expect(search).toBeFocused();
    expect(await search.evaluate((input) => input.parentElement!.getBoundingClientRect().left)).toBeCloseTo(empty.x, 0);
    await page.keyboard.press("Escape");
    await expect(search).toHaveCount(0);

    await settle(page);
});

test("frames", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await graphReady(page);
    await settle(page);
    const members = ["get_midi_file-1", "get_midi_track_data-1"];
    await frameNodes(page, members);

    // shift+p joins the selection in a new frame
    await page.keyboard.press("Shift+P");
    await expect.poll(async () => lastOp(page, "join_in_frame")).toEqual({ op: "join_in_frame", nodes: members });

    // the backend sends the frame back: it's drawn under its nodes, fitted around them, with its label
    const state = backend.get_state;
    const rf = structuredClone(state.rf_instance);
    for (const node of rf.nodes) {
        node.selected = false;
        if (members.includes(node.id)) node.frame = "frame-1";
    }
    // (the fake backend keeps no selection, so nothing's selected in it)
    rf.nodes.push({ id: "frame-1", type: "frame", position: { x: 0, y: 0 }, width: 100, height: 100, data: { label: "Drums", color: "#999999" } });
    await emitEvent(page, "graph_changed", { tab: state.active_tab, graph_rev: state.graph_rev + 1, rf_instance: rf });
    const frame = page.locator('.react-flow__node[data-id="frame-1"]');
    await expect(frame.locator(".graph-frame-label")).toHaveText("Drums");
    expect(await frame.locator(".graph-frame-label").evaluate((el) => [getComputedStyle(el).fontSize, getComputedStyle(el).fontWeight])).toEqual(["20px", "700"]);
    expect(Number(await frame.evaluate((el) => getComputedStyle(el).zIndex))).toBeLessThan(0);
    const box = (await frame.boundingBox())!;
    for (const id of members) {
        const node = (await page.locator(`.react-flow__node[data-id="${id}"]`).boundingBox())!;
        expect(node.x > box.x && node.y > box.y && node.x + node.width < box.x + box.width && node.y + node.height < box.y + box.height, `${id} isn't inside the frame`).toBe(true);
    }

    // a click on its empty inside selects it, its right click menu is the selection's
    const inside = await page.evaluate((box) => {
        for (let y = box.y + 40; y < box.y + box.height - 5; y += 8) {
            for (let x = box.x + 5; x < box.x + box.width - 5; x += 8) {
                if (document.elementFromPoint(x, y)?.classList.contains("react-flow__pane")) return { x, y };
            }
        }
        throw new Error("no empty point in the frame");
    }, box);
    const menu = page.locator(".context-menu");
    const rows = menu.locator(".context-menu-row");
    await page.locator('.react-flow__node[data-id="get_midi_file-1"] .node-header').click();
    await page.mouse.click(inside.x, inside.y);
    await expect(frame).toHaveClass(/selected/);
    await expect.poll(async () => (await sentOps(page)).filter((op: any) => op.op === "select").at(-1)).toMatchObject({ nodes: ["frame-1"] });
    await page.mouse.click(inside.x, inside.y, { button: "right" });
    await expect(rows).toHaveText(["Cut", "Copy", "Paste", "Duplicate", "Delete", "Join in New Frame", "Make Group"]);
    await page.keyboard.press("Escape");

    // a click that moves a pixel selects it too
    await page.locator('.react-flow__node[data-id="get_midi_file-1"] .node-header').click();
    await expect(frame).not.toHaveClass(/selected/);
    await page.mouse.move(inside.x, inside.y);
    await page.mouse.down();
    await page.mouse.move(inside.x + 1, inside.y + 1);
    await page.mouse.up();
    await expect(frame).toHaveClass(/selected/);
    await expect.poll(async () => (await sentOps(page)).filter((op: any) => op.op === "select").at(-1)).toMatchObject({ nodes: ["frame-1"] });

    // a node in it can be taken out, alt+p does the same
    await page.locator('.react-flow__node[data-id="get_midi_file-1"] .node-header').click({ button: "right" });
    await expect(rows).toHaveText(["Cut", "Copy", "Paste", "Duplicate", "Delete", "Join in New Frame", "Remove from Frame", "Make Group"]);
    await rows.getByText("Remove from Frame", { exact: true }).click();
    await expect.poll(async () => lastOp(page, "remove_from_frame")).toEqual({ op: "remove_from_frame", nodes: ["get_midi_file-1"] });
    await page.mouse.move(inside.x, inside.y);
    await page.keyboard.press("Alt+P");
    await expect.poll(async () => (await sentOps(page)).filter((op: any) => op.op === "remove_from_frame").length).toBe(2);

    // double clicking the label renames it
    const label = frame.locator(".graph-frame-label");
    await label.dblclick();
    await frame.locator(".graph-frame-rename").fill("Bass");
    await page.keyboard.press("Enter");
    await expect.poll(async () => lastOp(page, "set_label")).toEqual({ op: "set_label", node: "frame-1", label: "Bass" });

    // dragging the label moves the frame and what's in it, as one move
    const start = (await label.boundingBox())!;
    await page.mouse.move(start.x + 20, start.y + start.height / 2);
    await page.mouse.down();
    await page.mouse.move(start.x + 80, start.y + start.height / 2 + 40, { steps: 6 });
    await page.mouse.up();
    await expect.poll(async () => lastOp(page, "move")).toBeTruthy();
    const move = await lastOp(page, "move");
    expect(Object.keys(move.positions).sort()).toEqual(["frame-1", ...members].sort());
    const moved = (id: string) => move.positions[id].x - rf.nodes.find((n: any) => n.id === id).position.x;
    expect(moved("get_midi_file-1")).toBeCloseTo(moved("get_midi_track_data-1"), 3);
    expect(move.frames).toBeUndefined();

    // a node grabbed and placed with the cursor over the frame joins it
    rf.nodes.find((n: any) => n.id === "viewer-1").selected = true;
    await emitEvent(page, "graph_changed", { tab: state.active_tab, graph_rev: state.graph_rev + 2, rf_instance: rf });
    await expect(page.locator('.react-flow__node[data-id="viewer-1"]')).toHaveClass(/selected/);
    await page.mouse.move(inside.x, inside.y);
    await page.keyboard.press("g");
    await page.mouse.move(inside.x + 10, inside.y + 5, { steps: 3 });
    await page.mouse.click(inside.x + 10, inside.y + 5);
    await expect.poll(async () => (await lastOp(page, "move"))?.frames).toEqual({ "viewer-1": "frame-1" });

    // a node dropped on it from the nodes panel goes in it
    await page.evaluate(({ x, y }) => window.dispatchEvent(new CustomEvent("motionkeys:node-drop", { detail: { nodeType: "viewer", clientX: x, clientY: y, offsetX: 10, offsetY: 10 } })), inside);
    await expect.poll(async () => (await lastOp(page, "add_nodes"))?.nodes[0].frame).toBe("frame-1");

    // a node dragged out past it stays in it, the frame grows to take it in
    const header = page.locator('.react-flow__node[data-id="get_midi_file-1"] .node-header');
    const from = (await header.boundingBox())!;
    const around = (await frame.boundingBox())!;
    await page.mouse.move(from.x + 10, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(from.x + 15, from.y + from.height / 2);
    await page.mouse.move(around.x + around.width + 40, from.y + from.height / 2, { steps: 8 });
    await page.mouse.up();
    await expect.poll(async () => Object.keys((await lastOp(page, "move"))?.positions ?? {})).toEqual(["get_midi_file-1"]);
    expect((await lastOp(page, "move")).frames).toBeUndefined();

    // a box started inside it box-selects, the frame stays out of a box wholly inside it
    await page.mouse.move(inside.x, inside.y);
    await page.mouse.down();
    await page.mouse.move(inside.x + 4, inside.y + 4, { steps: 3 });
    await expect(page.locator(".react-flow__selection")).toHaveCount(1);
    await page.mouse.up();
    await expect(frame).not.toHaveClass(/selected/);

    // a locked frame stays behind the graph: a click inside it doesn't select it and its label doesn't move it, a
    // click on its label selects it
    rf.nodes.find((n: any) => n.id === "frame-1").data.locked = true;
    await emitEvent(page, "graph_changed", { tab: state.active_tab, graph_rev: state.graph_rev + 3, rf_instance: rf });
    await expect(frame).toHaveClass(/frame-locked/);
    await expect(frame.locator(".react-flow__resize-control")).toHaveCount(0);
    const lockedInside = await page.evaluate(
        (box) => {
            for (let y = box.y + 50; y < box.y + box.height - 5; y += 8) {
                for (let x = box.x + 5; x < box.x + box.width - 5; x += 8) {
                    if (document.elementFromPoint(x, y)?.classList.contains("react-flow__pane")) return { x, y };
                }
            }
            throw new Error("no empty point in the frame");
        },
        (await frame.boundingBox())!
    );
    await page.mouse.click(lockedInside.x, lockedInside.y);
    await page.waitForTimeout(200);
    await expect(frame).not.toHaveClass(/selected/);
    const moves = (await sentOps(page)).filter((op: any) => op.op === "move").length;
    const strip = (await label.boundingBox())!;
    await page.mouse.move(strip.x + 20, strip.y + strip.height / 2);
    await page.mouse.down();
    await page.mouse.move(strip.x + 80, strip.y + strip.height / 2 + 40, { steps: 6 });
    await page.mouse.up();
    await expect(frame).toHaveClass(/selected/);
    expect((await sentOps(page)).filter((op: any) => op.op === "move").length).toBe(moves);
    await settle(page);
});

test("frame color picker", async ({ page }) => {
    await openWindow(page, withLayout(ALL_DOCKED), "/#/");
    await graphReady(page);
    await settle(page);
    const state = backend.get_state;
    const rf = structuredClone(state.rf_instance);
    for (const node of rf.nodes) node.selected = false;
    rf.nodes.find((n: any) => n.id === "get_midi_file-1").frame = "frame-1";
    rf.nodes.push({ id: "frame-1", type: "frame", position: { x: 0, y: 0 }, data: { color: "#e7e7e7" }, selected: true });
    await emitEvent(page, "graph_changed", { tab: state.active_tab, graph_rev: state.graph_rev + 1, rf_instance: rf });

    // the frame's color row has its hex code, typing one sets it
    const rowHex = page.locator(".properties .color-field input");
    await expect(rowHex).toHaveValue("#E7E7E7");
    await rowHex.fill("aa3333");
    await rowHex.press("Enter");
    await expect.poll(async () => (await sentOps(page)).filter((op: any) => op.op === "set_frame").at(-1)).toMatchObject({ node: "frame-1", color: "#aa3333" });

    // its square opens the picker: the wheel with the value beside it, the channels as sliders and the hex
    await page.locator(".color-field .color-swatch").click();
    const picker = page.locator(".color-popover");
    await expect(picker.locator(".w-color-wheel")).toBeVisible();
    await expect(picker.locator(".color-picker-slider")).toHaveText(["Hue0.000", "Saturation0.000", "Value0.906"]);
    await expect(picker.locator(".color-picker-hex input")).toHaveValue("#E7E7E7");
    const wheel = (await picker.locator(".w-color-wheel").boundingBox())!;
    const slider = (await picker.locator(".w-color-saturation").boundingBox())!;
    expect(slider.x).toBeGreaterThan(wheel.x + wheel.width);
    expect(Math.abs(slider.y - wheel.y)).toBeLessThan(2);

    // every change while it's open is one transaction, sent as a hex code
    const colors = async () => (await sentOps(page)).filter((op: any) => op.op === "set_frame").map((op: any) => op.color);
    // red at the bottom of the wheel like blender's, the value slider bright at the top
    await page.mouse.click(wheel.x + wheel.width / 2, wheel.y + wheel.height - 8);
    await expect.poll(async () => (await colors()).at(-1)).toMatch(/^#e[0-9a-f]{3}[0-3][0-9a-f]$/);
    await page.mouse.click(slider.x + slider.width / 2, slider.y + 2);
    await expect.poll(async () => (await colors()).at(-1)).toMatch(/^#f[0-9a-f]{5}$/);
    const handle = (await picker.locator(".w-color-saturation [class*=pointer]").first().boundingBox())!;
    expect(handle.y - slider.y).toBeLessThan(6);
    await picker.getByText("RGB", { exact: true }).click();
    await expect(picker.locator(".color-picker-slider").first()).toContainText("Red");
    const hex = picker.locator(".color-picker-hex input");
    await hex.fill("#336699");
    await hex.press("Enter");
    await expect.poll(async () => (await colors()).at(-1)).toBe("#336699");
    await expect(picker.locator(".color-picker-slider")).toHaveText(["Red0.200", "Green0.400", "Blue0.600"]);
    const txns = await page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "graph_apply" && c.args.ops.some((op: any) => op.op === "set_frame") && c.args.txn).map((c: any) => c.args.txn));
    expect(new Set(txns).size).toBe(1);
    expect(txns[0]).toBeTruthy();

    // escape closes it and ends the transaction
    await page.keyboard.press("Escape");
    await expect(picker).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => (window as any).__smokeCalls.filter((c: any) => c.cmd === "history_end").map((c: any) => c.args.txn))).toContain(txns[0]);
    await settle(page);
});

test("right click cancels a modal and opens no menu", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    await graphReady(page);
    await settle(page);
    const menu = page.locator(".context-menu");
    const noMenu = async () => {
        await page.waitForTimeout(200);
        await expect(menu).toHaveCount(0);
    };
    const node = page.locator('.react-flow__node[data-id="get_midi_track_data-1"]');

    // a grab puts the node back, nothing moved it
    await node.locator(".node-header").click();
    const before = (await node.boundingBox())!;
    const empty = await panePoint(page);
    await page.mouse.move(empty.x, empty.y);
    await page.keyboard.press("g");
    await page.mouse.move(empty.x + 60, empty.y + 40, { steps: 4 });
    await expect.poll(async () => (await node.boundingBox())!.x).not.toBe(before.x);
    await page.mouse.click(empty.x + 60, empty.y + 40, { button: "right" });
    await expect.poll(async () => (await node.boundingBox())!.x).toBe(before.x);
    await noMenu();
    expect((await sentOps(page)).some((op: any) => op.op === "move")).toBe(false);

    // the add menu closes
    await page.mouse.move(empty.x, empty.y);
    await page.keyboard.press("Shift+A");
    const search = page.getByPlaceholder("Search nodes...");
    await expect(search).toBeFocused();
    const outside = await panePoint(page, (await search.locator("..").boundingBox())!);
    await page.mouse.click(outside.x, outside.y, { button: "right" });
    await expect(search).toHaveCount(0);
    await noMenu();

    // a box select goes, the selection is what it was before it
    await page.mouse.move(empty.x, empty.y);
    await page.mouse.down();
    await page.mouse.move(empty.x + 300, empty.y + 300, { steps: 8 });
    await page.mouse.down({ button: "right" });
    await expect(page.locator(".react-flow__selection")).toHaveCount(0);
    await page.mouse.up({ button: "right" });
    await page.mouse.move(empty.x + 320, empty.y + 320, { steps: 2 });
    await page.mouse.up();
    await noMenu();
    await expect(page.locator(".react-flow__node.selected")).toHaveCount(1);
    await expect(node).toHaveClass(/selected/);
    await expect(page.locator(".react-flow__edge.selected")).toHaveCount(0);

    // the tag menu closes
    const output = node.locator(".react-flow__handle.target").first();
    await output.dblclick();
    const tags = page.locator(".tag-menu");
    await expect(tags).toBeVisible();
    const away = await panePoint(page, (await tags.boundingBox())!);
    await page.mouse.click(away.x, away.y, { button: "right" });
    await expect(tags).toHaveCount(0);
    await noMenu();

    // a link dragged off an output connects nothing, and no add menu opens where it's let go
    const connects = (await sentOps(page)).filter((op: any) => op.op === "connect").length;
    const socket = (await output.boundingBox())!;
    await page.mouse.move(socket.x + socket.width / 2, socket.y + socket.height / 2);
    await page.mouse.down();
    await page.mouse.move(socket.x + 150, socket.y + 80, { steps: 8 });
    await expect(page.locator(".react-flow__connectionline")).toHaveCount(1);
    await page.mouse.down({ button: "right" });
    await expect(page.locator(".react-flow__connectionline")).toHaveCount(0);
    await page.mouse.up({ button: "right" });
    await page.mouse.move(socket.x + 170, socket.y + 90, { steps: 2 });
    await page.mouse.up();
    await noMenu();
    await expect(search).toHaveCount(0);
    expect((await sentOps(page)).filter((op: any) => op.op === "connect").length).toBe(connects);
    await settle(page);
});
