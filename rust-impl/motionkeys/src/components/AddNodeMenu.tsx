import { useCallback, useEffect, useState } from "react";
import { Button } from "glisse";
import { Icon } from "../icons";
import { TOOLBAR_ADD_EVENT, useEditorState } from "../utils/editorState";
import NodeAddMenu from "./nodegraph/NodeAddMenu";
import { layoutRect } from "./GlassMenu";

// the toolbar's add button, the add menu (the same as shift+a's) under it. the picked node is added under the cursor and
// grabbed (NodeGraphEditor)
function AddNodeMenu() {
    const { editable, entries } = useEditorState();
    const enabled = editable && !!entries;
    // where the menu opened, read once from the button's place without its press animation so the menu holds still
    const [at, setAt] = useState<{ x: number; y: number } | null>(null);

    const close = useCallback(() => setAt(null), []);

    // closes when clicked outside, or when the graph on screen can't take a node
    useEffect(() => {
        if (!at) return;
        window.addEventListener("mousedown", close);
        return () => window.removeEventListener("mousedown", close);
    }, [at, close]);
    useEffect(() => {
        if (!enabled) close();
    }, [enabled, close]);

    const toggle = (button: HTMLElement) => {
        if (at) return close();
        const rect = layoutRect(button);
        setAt({ x: rect.left, y: rect.bottom + 6 });
    };

    const pick = (key: string) => {
        close();
        window.dispatchEvent(new CustomEvent(TOOLBAR_ADD_EVENT, { detail: key }));
    };

    return (
        <>
            <Button className={`toolbar-button${enabled ? "" : " disabled"}`} disabled={!enabled} data-tool="add_node" onMouseDown={(e) => e.stopPropagation()} onClick={(e) => toggle(e.currentTarget)} icon={<Icon name="add" className="tool-icon" />}>
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={3} stroke="currentColor" className="tool-chevron">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 9l7 7 7-7" />
                </svg>
            </Button>
            {at && entries && <NodeAddMenu entries={entries} x={at.x} y={at.y} onSelect={pick} onClose={close} />}
        </>
    );
}

export default AddNodeMenu;
