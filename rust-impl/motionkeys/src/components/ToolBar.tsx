import { useEffect, useRef, useState } from "react";
import Tool, { CommandTool } from "./Tool";
import AddNodeMenu from "./AddNodeMenu";
import { useStateContext } from "../contexts/StateContext";
import { useEditorState } from "../utils/editorState";
import { Glass } from "glisse";

function MenuBar() {
    const { frontEndState, setFrontEndState } = useStateContext();
    // which of the node editor's buttons can run in the graph on screen
    const editor = useEditorState();

    // webkit blurs an icon that sits between screen pixels. the right hand buttons are placed from the toolbar's right edge,
    // which does whenever the window's width does, so they move left onto a whole pixel
    const rightRef = useRef<HTMLDivElement>(null);
    const [snap, setSnap] = useState(0);
    const snapRef = useRef(0);
    useEffect(() => {
        const right = rightRef.current;
        const bar = right?.parentElement;
        if (!right || !bar) return;
        const measure = () => {
            const ratio = window.devicePixelRatio || 1;
            const edge = (right.getBoundingClientRect().right + snapRef.current) * ratio;
            snapRef.current = (edge - Math.floor(edge)) / ratio;
            setSnap(snapRef.current);
        };
        const observer = new ResizeObserver(measure);
        observer.observe(bar);
        return () => observer.disconnect();
    }, []);

    // shows or hides everything docked on a side
    const collapse = (side: "left" | "right") => {
        setFrontEndState((prev: any) => ({
            ...prev,
            sidesHidden: prev.sidesHidden.includes(side) ? prev.sidesHidden.filter((s: string) => s !== side) : [...prev.sidesHidden, side],
        }));
    };
    const collapseLeft = () => collapse("left");
    const collapseRight = () => collapse("right");

    return (
        <div className="toolbar flex h-8 items-center gap-1.5 px-1">
            {/* logo, in a button's box. a whole number of pixels wide so the icons after it stay on whole pixels */}
            <div className="logo toolbar-button">
                <img src="logo.webp" alt="logo" className="size-6" />
            </div>

            {/* each group of buttons shares one glass capsule, like a macOS toolbar */}
            <Glass className="tool-group">
                <Tool type="collapse-left" onClick={collapseLeft} />
                <Tool type="save" />
                <Tool type="load" />
            </Glass>

            <AddNodeMenu />

            <Glass className="tool-group">
                <CommandTool icon="undo" command="undo" />
                <CommandTool icon="redo" command="redo" />
            </Glass>

            {/* the node editor's commands, the same as the node and view menus */}
            <Glass className="tool-group">
                <CommandTool icon="group" command="area.group" disabled={!editor.editable || editor.selected === 0 || !editor.entries} />
                <CommandTool icon="ungroup" command="area.ungroup" disabled={!editor.editable || editor.groups === 0} />
            </Glass>
            <Glass className="tool-group">
                <CommandTool icon="edit_group" command="area.edit_group" disabled={editor.openable !== 1} />
                <CommandTool icon="exit_group" command="area.exit_group" disabled={!editor.inGroup} />
                <CommandTool icon="exit_to_root" command="area.exit_to_root" disabled={!editor.inGroup} />
            </Glass>
            <Glass className="tool-group">
                <CommandTool icon="frame_all" command="area.frame_all" />
                <CommandTool icon="frame_selected" command="area.frame_selected" />
            </Glass>

            {/* right aligned items */}
            <div ref={rightRef} className="ml-auto flex gap-1.5" style={{ marginRight: snap }}>
                <Tool type="run" />
                <Tool type="collapse-right" onClick={collapseRight} />
            </div>
        </div>
    );
}

export default MenuBar;
