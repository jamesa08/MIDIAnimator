import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
    plugins: [react()],
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
