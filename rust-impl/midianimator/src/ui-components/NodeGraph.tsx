// @ts-nocheck
import ReactFlow, { Controls, Background, PanOnScrollMode } from "reactflow";
import "reactflow/dist/style.css";

function NodeGraph() {
    function detectTrackPad(e) {
        var isTrackpad = false;
        const { deltaY } = e;
        if (deltaY && !Number.isInteger(deltaY)) {
            isTrackpad = true;
        }
        isTrackpad = false;
        console.log(isTrackpad ? "Trackpad detected" : "Mousewheel detected");
    }

    document.addEventListener("mousewheel", detectTrackPad, false);
    document.addEventListener("DOMMouseScroll", detectTrackPad, false);

    return (
        <ReactFlow
            panOnScroll={true}
            // panOnScrollMode={PanOnScrollMode.Vertical}
        >
            <Background />
            <Controls />
        </ReactFlow>
    );
}

export default NodeGraph;
