import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { HashRouter as Router, Route, Routes } from "react-router-dom";
import reportWebVitals from "./reportWebVitals";
import "./index.css";
import PanelContent from "./components/PanelContent";
import Settings from "./windows/Settings";
import StateContextProvider from "./contexts/StateContext";

const rootElement = document.getElementById("root");

if (rootElement) {
    const root = ReactDOM.createRoot(rootElement);
    root.render(
        <React.StrictMode>
            <StateContextProvider>
                <Router>
                    <Routes>
                        <Route path="/" element={<App />} />
                        <Route path="/panel/:id" element={<PanelContent />} />
                        <Route path="/settings" element={<Settings />} />
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
