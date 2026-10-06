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

test("drag ghost window", async ({ page }) => {
    await openWindow(page, backend, "/#/drag-ghost", "drag-ghost");
    await settle(page);
});

test("splash window", async ({ page }) => {
    await openWindow(page, backend, "/splash.html", "splashscreen");
    await settle(page);
});
