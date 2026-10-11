import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export const StateContext = createContext<StateContext | null>(null);

const defaultBackendState = { ready: false };
// a tab's window layout. panelsShown: open panels, docked or floating, in the order they were docked. panelsPoppedOut:
// panels that float (a closed one floats again when it's opened). panelSides: the side each panel docks on. sidesHidden:
// sides collapsed from the toolbar. dockWidths: each side's docked panel width (without the scrollbar), dragged from the
// column's inner edge. floating: where each floating panel's window was, screen pixels. the history panel
// starts closed and floating
export const defaultLayout = { panelsShown: [0, 1], panelsPoppedOut: [2] as number[], panelSides: {} as Record<number, "left" | "right">, sidesHidden: [] as ("left" | "right")[], dockWidths: {} as Partial<Record<"left" | "right", number>>, floating: {} as Record<number, { x: number; y: number; width: number; height: number }> };

type StateContextProviderProps = {
    children: React.ReactNode;
};
type StateContext = {
    backEndState: any;
    setBackEndState: React.Dispatch<React.SetStateAction<any>>;
    // the window layout of the tab on screen
    frontEndState: any;
    setFrontEndState: React.Dispatch<React.SetStateAction<any>>;
    // the window layout of any tab
    setTabLayout: (tab: string, update: React.SetStateAction<any>) => void;
};

// a layout from a file, filled in where it's missing something
const withDefaults = (layout: any) => ({ ...defaultLayout, ...(layout && typeof layout === "object" ? layout : {}) });

// create a context provider
const StateContextProvider = ({ children }: StateContextProviderProps) => {
    const [backendState, setBackEndState] = useState<any>(defaultBackendState);
    // each tab's window layout, by tab id. a tab not in here yet starts with the layout the backend has for it (from its
    // file), or the default one
    const [layouts, setLayouts] = useState<Record<string, any>>({});
    const tab: string = backendState.active_tab ?? "";
    const frontendState = useMemo(() => layouts[tab] ?? withDefaults(backendState.layout), [layouts, tab, backendState.layout]);

    // for the setters, which listeners keep from their first render
    const latest = useRef({ tab, backendLayout: backendState.layout });
    latest.current = { tab, backendLayout: backendState.layout };

    const setTabLayout = useCallback((id: string, update: React.SetStateAction<any>) => {
        setLayouts((prev) => {
            const current = prev[id] ?? withDefaults(id === latest.current.tab ? latest.current.backendLayout : null);
            const next = typeof update === "function" ? (update as (prev: any) => any)(current) : update;
            return { ...prev, [id]: next };
        });
    }, []);
    const setFrontEndState = useCallback((update: React.SetStateAction<any>) => setTabLayout(latest.current.tab, update), [setTabLayout]);

    // the backend keeps each tab's layout to save it with the tab's file
    const sent = useRef<Record<string, any>>({});
    useEffect(() => {
        for (const [id, layout] of Object.entries(layouts)) {
            if (sent.current[id] === layout) continue;
            sent.current[id] = layout;
            invoke("set_layout", { tab: id, layout }).catch(() => {});
        }
    }, [layouts]);

    return <StateContext.Provider value={{ backEndState: backendState, setBackEndState: setBackEndState, frontEndState: frontendState, setFrontEndState: setFrontEndState, setTabLayout }}>{children}</StateContext.Provider>;
};

// custom state hook
export const useStateContext = () => {
    const contextObj = useContext(StateContext);

    if (!contextObj) {
        throw new Error("useStateContext must be used within a StateContextProvider");
    }

    return contextObj;
};

export default StateContextProvider;
