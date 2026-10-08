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

test("main window with every panel docked", async ({ page }) => {
    await openWindow(page, withLayout(ALL_DOCKED), "/#/");
    await expect(page.locator(".dock-column .nodes-grid .node.preview").first()).toBeVisible();
    await expect(page.getByText("Move", { exact: true })).toBeVisible();
    await settle(page);
});

test("resizing the docked panels", async ({ page }) => {
    await openWindow(page, withLayout({ ...ALL_DOCKED, panelSides: { 2: "left" } }), "/#/");
    for (const [side, dx] of [["left", 100], ["right", -100]] as const) {
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
    for (const [typed, note] of [["70", 70], ["c#4", 73]] as const) {
        await page.keyboard.press("Shift+A");
        await expect(box).toBeFocused();
        await page.keyboard.type(typed);
        await page.keyboard.press("Enter");
        await expect(box).toHaveCount(0);
        const added = (await addedNotes()).at(-1);
        expect(added.inputs.mode).toBe("map");
        expect(added.inputs.note_map.notes).toContain(note);
        expect(added.inputs.map_layout[`n:${note}`]).toBeTruthy();
        // the fake backend doesn't add it, escape cancels the grab
        await page.keyboard.press("Escape");
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

test("drag ghost window", async ({ page }) => {
    await openWindow(page, backend, "/#/drag-ghost", "drag-ghost");
    await settle(page);
});

test("splash window", async ({ page }) => {
    await openWindow(page, backend, "/splash.html", "splashscreen");
    await settle(page);
});
