import { useEffect } from "react";
import { useStoreApi } from "@xyflow/react";
import { addsToSelection } from "../../utils/keymap";

// how far the mouse can move between press and release and still be a click, further is a drag (blender's 3px). the
// same for both, react flow's defaults (a drag past 1px, a click only with no movement at all) left a click that moved a
// pixel as neither, it selected nothing
export const CLICK_DISTANCE = 3;

// multi select for a graph (the editor, a note map) while shift or cmd (ctrl off a mac) is held, tracked here instead
// of multiSelectionKeyCode. react flow ignores a keyup inside an input, so releasing shift after shift+a focused the
// add menu search left multi select stuck on (clicking another node added to the selection).
// reading the modifiers off every key/mouse event (addsToSelection) means it can't get stuck
export function useMultiSelection() {
    const store = useStoreApi();
    useEffect(() => {
        const setMultiSelection = (active: boolean) => {
            if (store.getState().multiSelectionActive !== active) {
                store.setState({ multiSelectionActive: active });
            }
        };
        const handleEvent = (event: KeyboardEvent | MouseEvent) => setMultiSelection(addsToSelection(event));
        const handleBlur = () => setMultiSelection(false);

        // capture phase so it's set before react flow handles the click
        window.addEventListener("keydown", handleEvent, true);
        window.addEventListener("keyup", handleEvent, true);
        window.addEventListener("pointerdown", handleEvent, true);
        window.addEventListener("mousedown", handleEvent, true);
        window.addEventListener("blur", handleBlur);
        return () => {
            window.removeEventListener("keydown", handleEvent, true);
            window.removeEventListener("keyup", handleEvent, true);
            window.removeEventListener("pointerdown", handleEvent, true);
            window.removeEventListener("mousedown", handleEvent, true);
            window.removeEventListener("blur", handleBlur);
        };
    }, [store]);
}
