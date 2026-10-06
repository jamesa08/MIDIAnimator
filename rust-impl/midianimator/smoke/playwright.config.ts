import { defineConfig, devices } from "@playwright/test";

// its own port, so it doesn't clash with a running `tauri dev`
const PORT = 3123;

export default defineConfig({
    testDir: ".",
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    reporter: process.env.CI ? "github" : "list",
    use: { baseURL: `http://localhost:${PORT}`, viewport: { width: 1280, height: 800 } },
    projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } }],
    // the dev server, so react runs in dev mode (strict mode's double effects, its warnings)
    webServer: { command: `npx vite --port ${PORT} --strictPort`, cwd: "..", url: `http://localhost:${PORT}`, reuseExistingServer: !process.env.CI },
});
