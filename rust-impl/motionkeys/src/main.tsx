import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { HashRouter as Router, Route, Routes } from "react-router-dom";
import reportWebVitals from "./reportWebVitals";
import "./index.css";
import PanelContent from "./components/PanelContent";
import Settings from "./windows/Settings";
import Graph from "./windows/Graph";
import DragGhost from "./windows/DragGhost";
import StateContextProvider from "./contexts/StateContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme } from "./utils/theme";
import "./utils/liveResize";
import { scrollbarWidth } from "./utils/panels";
import "./utils/keymap";
import "./utils/editMenu";
import ErrorBoundary from "./components/ErrorBoundary";
import { captureConsole } from "./utils/console";

// debug mode sends this window's console output to the console (src/utils/console.ts)
captureConsole();

// tells the backend this window has drawn, new windows stay invisible until then (src-tauri/src/ui/windows.rs)
function WindowReady() {
    React.useEffect(() => {
        requestAnimationFrame(() => requestAnimationFrame(() => invoke("window_ready")));
    }, []);
    return null;
}

// loads the appearance.theme setting's css (utils/theme.ts) and the page zoom for what keeps its size on screen
// (index.css --zoom), and again whenever they change
function ThemeSync() {
    React.useEffect(() => {
        const apply = (settings: any) => {
            applyTheme(settings?.appearance?.theme ?? "light");
            document.documentElement.style.setProperty("--zoom", String(settings?.appearance?.zoom ?? 1));
        };
        invoke("get_settings").then(apply);
        const unlisten = listen("settings_changed", (event: any) => apply(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);
    return null;
}

// errors nothing caught (a throw in an event handler, a rejected invoke without a catch) go to the backend's log too, the
// console is gone in a release build. they don't take the window down, only one thrown while drawing does (ErrorBoundary)
window.addEventListener("error", (event) => {
    invoke("log", { message: `uncaught error in ${window.location.hash || "/"}: ${event.message}\n${event.error?.stack ?? ""}` }).catch(() => {});
});
window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason instanceof Error ? `${event.reason.message}\n${event.reason.stack ?? ""}` : String(event.reason);
    invoke("log", { message: `unhandled rejection in ${window.location.hash || "/"}: ${reason}` }).catch(() => {});
});

// a window's page in a boundary of its own, so a part that has none of its own shows its error instead of a blank window
const guarded = (name: string, page: React.ReactNode) => <ErrorBoundary name={name}>{page}</ErrorBoundary>;

// the dock columns and floating panels make room for a scrollbar beside their contents (index.css .dock-width)
document.documentElement.style.setProperty("--scrollbar-width", `${scrollbarWidth()}px`);

const rootElement = document.getElementById("root");

if (rootElement) {
    const root = ReactDOM.createRoot(rootElement);
    root.render(
        <React.StrictMode>
            <StateContextProvider>
                <WindowReady />
                <ThemeSync />
                <Router>
                    <Routes>
                        <Route path="/" element={guarded("Main Window", <App />)} />
                        <Route path="/panel/:id" element={guarded("Panel", <PanelContent />)} />
                        <Route path="/settings" element={guarded("Settings", <Settings />)} />
                        <Route path="/graph" element={guarded("Graph", <Graph />)} />
                        <Route path="/drag-ghost" element={guarded("Drag Ghost", <DragGhost />)} />
                    </Routes>
                </Router>
            </StateContextProvider>
        </React.StrictMode>
    );
}

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals(console.log);
