import MenuBar from "./components/MenuBar";
import ToolBar from "./components/ToolBar";
import Panel from "./components/Panel";
import NodeGraph from "./components/NodeGraph";
import StatusBar from "./components/StatusBar";
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/tauri";

import { useStateContext } from "./contexts/StateContext";

function App() {
    const { backEndState: backEndState, setBackEndState: setBackEndState, frontEndState: frontEndState, setFrontEndState: setFrontEndState } = useStateContext();

    useEffect(() => {
        // listner for window creation
        const windowEventListener = listen(`open-window`, (event: any) => {
            const window = new WebviewWindow(`${event.payload["title"]}`, event.payload);

            window.show();
        });

        const stateListner = listen("update_state", (event: any) => {
            setBackEndState(event.payload);
        });

        // tell the backend we're ready & get the initial state
        invoke("ready").then((res: any) => {
            if (res !== null) {
                setBackEndState(res);
            }
        });

        return () => {
            windowEventListener.then((f) => f());
            stateListner.then((f) => f());
        };
    }, []);

    return (
        <div className="wrapper w-screen h-screen overflow-hidden flex flex-col">
            <div className="head flex-initial">
                <MenuBar />
                <ToolBar />
            </div>
            <div className="content flex flex-auto ">
                <Panel id="0" name="Nodes" />
                <div className="node-graph flex-grow border-black border-l border-r">
                    <NodeGraph />
                </div>
                <div className="ml-auto flex">
                    <Panel id="1" name="Properties" />
                </div>
            </div>
            <div className="foot flex-initial">
                <StatusBar event="some info here" />
            </div>
        </div>
    );
}

export default App;
