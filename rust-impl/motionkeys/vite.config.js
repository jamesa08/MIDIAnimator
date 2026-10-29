import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";

export default defineConfig({
    plugins: [react()],
    // glisse is vendored (src/vendor/glisse) for its faster refraction, imported by its package name
    resolve: {
        alias: { glisse: fileURLToPath(new URL("./src/vendor/glisse/index.ts", import.meta.url)) },
    },
    build: {
        outDir: "build",
        rollupOptions: {
            input: {
                main: "index.html",
                splash: "splash.html",
            },
        },
    },
    server: {
        port: 3000,
    },
});
