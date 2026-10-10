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

// the graph on screen is ready: until then it's drawn see-through and takes no clicks (NodeGraph.tsx). a locator's click
// waits for that, a click at a point with the mouse doesn't, and opacity 0 still counts as visible
async function graphReady(page: Page) {
    await expect(page.locator('.graph-layer:not([style*="opacity"]) .react-flow__node').first()).toBeAttached();
}

// a point on an edge's path on screen where it's the edge a click gets, the nearest to its middle. another edge or a
// node can be over the middle depending on how the nodes measure (fonts differ between platforms)
async function edgePoint(edge: ReturnType<Page["locator"]>) {
    return edge.evaluate((g: Element) => {
        const path = g.querySelector<SVGPathElement>("path.react-flow__edge-path")!;
        const ctm = path.getScreenCTM()!;
        const at = (fraction: number) => {
            const point = path.getPointAtLength(path.getTotalLength() * fraction);
            return { x: point.x * ctm.a + ctm.e, y: point.y * ctm.d + ctm.f };
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

test("box select around sockets only selects the sockets", async ({ page }) => {
    await openWindow(page, backend, "/#/");
    const node = page.locator('.react-flow__node[data-id="get_midi_track_data-1"]');
    await expect(node).toBeVisible();
    await graphReady(page);
    await settle(page);

    // a narrow box down the outputs, from above the node to the aftertouch socket. it touches the node's edge, but it's
    // around sockets and no whole node
    const bounds = (await node.boundingBox())!;
    const aftertouch = (await socketOf(page, "get_midi_track_data-1", "aftertouch").boundingBox())!;
    const right = bounds.x + bounds.width;
    await page.mouse.move(right + 20, bounds.y - 10);
    await page.mouse.down();
    await page.mouse.move(right - 3, aftertouch.y + aftertouch.height / 2 + 2, { steps: 8 });
    await page.mouse.up();
    for (const socket of ["notes", "control_change", "pitchwheel", "aftertouch"]) await expect(socketOf(page, "get_midi_track_data-1", socket)).toHaveClass(/socket-selected/);
    await expect(page.locator(".socket-selected")).toHaveCount(4);
    await expect(page.locator(".react-flow__node.selected")).toHaveCount(0);
    await expect(page.locator(".react-flow__edge.selected")).toHaveCount(0);
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
