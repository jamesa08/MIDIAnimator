import { createContext, useContext, useState } from "react";

export const StateContext = createContext<StateContext | null>(null);

const defaultBackendState = { ready: false };
// panelsShown: open panels, docked or floating, in the order they were docked. panelsPoppedOut: panels that float.
// panelSides: the side each panel docks on. sidesHidden: sides collapsed from the toolbar
const defaultFrontendState = { panelsShown: [0, 1], panelsPoppedOut: [] as number[], panelSides: {} as Record<number, "left" | "right">, sidesHidden: [] as ("left" | "right")[] };

type StateContextProviderProps = {
    children: React.ReactNode;
};
type StateContext = {
    backEndState: any;
    setBackEndState: React.Dispatch<React.SetStateAction<any>>;
    frontEndState: any;
    setFrontEndState: React.Dispatch<React.SetStateAction<any>>;
};

// create a context provider
const StateContextProvider = ({ children }: StateContextProviderProps) => {
    const [backendState, setBackEndState] = useState(defaultBackendState);
    const [frontendState, setFrontEndState] = useState(defaultFrontendState);   

    return <StateContext.Provider value={{ backEndState: backendState, setBackEndState: setBackEndState, frontEndState: frontendState, setFrontEndState: setFrontEndState }}>{children}</StateContext.Provider>;
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
