import { useModal } from "../utils/keymap";
import { Button, Glass } from "glisse";

interface CurveImportModalProps {
    objects: Record<string, number>;
    onImport: () => void;
    onCancel: () => void;
}

// asked before importing the keyframes of objects over the keyframe limit (src-tauri/src/blender/curves.rs)
function CurveImportModal({ objects, onImport, onCancel }: CurveImportModalProps) {
    // takes the keys while it's up: cancel, or confirm which imports unless a button is focused (that one's pressed)
    useModal(
        "dialog",
        {
            confirm: () => (document.activeElement instanceof HTMLButtonElement ? document.activeElement.click() : onImport()),
            cancel: onCancel,
        },
        true,
        { passthrough: true }
    );

    return (
        <div className="fixed inset-0 bg-black/20 flex items-center justify-center z-[9999] font-[Arial,sans-serif] select-none" onClick={onCancel}>
            <Glass className="glass-dialog w-80" onClick={(e) => e.stopPropagation()}>
                <div className="h-7 flex items-center px-3 pt-2 font-bold">Import keyframes</div>
                <div className="px-3 py-3 text-sm">
                    {Object.entries(objects).map(([name, count]) => (
                        <p key={name}>{`${name}: ${count.toLocaleString()} keyframes`}</p>
                    ))}
                </div>
                <div className="flex gap-2 justify-end px-3 pb-3">
                    <Button onClick={onCancel}>Cancel</Button>
                    <Button variant="filled" onClick={onImport}>
                        Import
                    </Button>
                </div>
            </Glass>
        </div>
    );
}

export default CurveImportModal;
