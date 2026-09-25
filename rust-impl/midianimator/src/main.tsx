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

// tells the backend this window has drawn, new windows stay invisible until then (src-tauri/src/ui/windows.rs)
function WindowReady() {
    React.useEffect(() => {
        requestAnimationFrame(() => requestAnimationFrame(() => invoke("window_ready")));
    }, []);
    return null;
}

const rootElement = document.getElementById("root");

if (rootElement) {
    const root = ReactDOM.createRoot(rootElement);
    root.render(
        <React.StrictMode>
            <StateContextProvider>
                <WindowReady />
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
