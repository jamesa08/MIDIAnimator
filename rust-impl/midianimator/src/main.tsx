import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { HashRouter as Router, Route, Routes } from "react-router-dom";
import reportWebVitals from "./reportWebVitals";
import "./index.css";
import PanelContent from "./components/PanelContent";
import Settings from "./windows/Settings";
import DragGhost from "./windows/DragGhost";
import StateContextProvider from "./contexts/StateContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme } from "./utils/theme";
import "./utils/liveResize";
import { scrollbarWidth } from "./utils/panels";
import "./utils/keymap";
import "./utils/editMenu";

// tells the backend this window has drawn, new windows stay invisible until then (src-tauri/src/ui/windows.rs)
function WindowReady() {
    React.useEffect(() => {
        requestAnimationFrame(() => requestAnimationFrame(() => invoke("window_ready")));
    }, []);
    return null;
}

// loads the appearance.theme setting's css (utils/theme.ts), and again whenever it changes
function ThemeSync() {
    React.useEffect(() => {
        const apply = (settings: any) => applyTheme(settings?.appearance?.theme ?? "light");
        invoke("get_settings").then(apply);
        const unlisten = listen("settings_changed", (event: any) => apply(event.payload));
        return () => {
            unlisten.then((f) => f());
        };
    }, []);
    return null;
}

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
                        <Route path="/" element={<App />} />
                        <Route path="/panel/:id" element={<PanelContent />} />
                        <Route path="/settings" element={<Settings />} />
                        <Route path="/drag-ghost" element={<DragGhost />} />
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
