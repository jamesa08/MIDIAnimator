import MenuBar from "./ui-components/MenuBar";
import ToolBar from "./ui-components/ToolBar";
import Panel from "./ui-components/Panel";
import NodeGraph from "./ui-components/NodeGraph";
import StatusBar from "./ui-components/StatusBar";

function App() {
    return (
        <div className="wrapper w-screen h-screen overflow-hidden flex flex-col">
            <div className="head flex-initial">
                <MenuBar />
                <ToolBar />
            </div>
            <div className="content flex flex-auto ">
                <Panel name="Nodes" />
                <div className="node-graph flex-grow border-black border-l border-r">
                    <NodeGraph />
                </div>
                <div className="ml-auto flex">
                    <Panel name="Properties" />
                </div>
            </div>
            <div className="foot flex-initial">
                <StatusBar event="some info here"/>
            </div>
        </div>
    );
}

export default App;
