import { createContext, useContext, useState } from "react";

export const StateContext = createContext<StateContext | null>(null);

const defaultState = { "ready": false };

type StateContextProviderProps = {
    children: React.ReactNode;
};

type StateContext = {
    state: any;
    setState: React.Dispatch<React.SetStateAction<any>>;
};

const StateContextProvider = ({ children }: StateContextProviderProps) => {
    const [state, setState] = useState(defaultState);

    return <StateContext.Provider value={{ state, setState }}>{children}</StateContext.Provider>;
};

export const useStateContext = () => {
    const contextObj = useContext(StateContext);

    if (!contextObj) {
        throw new Error("useStateContext must be used within a StateContextProvider");
    }

    return contextObj;
};

export default StateContextProvider;
