import { invoke } from "@tauri-apps/api/tauri";

function Tool({ type }: { type: string }) {
    var icon;
    if (type == "run") {
        icon = (
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-6">
                <path strokeLinecap="round" strokeLinejoin="round" d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.347a1.125 1.125 0 0 1 0 1.972l-11.54 6.347a1.125 1.125 0 0 1-1.667-.986V5.653Z" />
            </svg>
        );
    } else if (type == "collapse-left") {
        icon = <img src="src/collapse-left.png" alt="collapse-left" className="size-6 h-5" />;
    } else if (type == "collapse-right") {
        icon = <img src="src/collapse-right.png" alt="collapse-right" className="size-6 h-5" />;
    }

    return (
        <div
            className="toolbar-button flex items-center pr-1 pl-1"
            onClick={() => {
                if (type == "run") {
                    invoke("execute_graph", { realtime: false });
                }
            }}
        >
            {icon}
        </div>
    );
}

export default Tool;
