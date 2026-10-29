import { useModal } from "../utils/keymap";
import { Button, Glass } from "glisse";

interface UnsavedChangesModalProps {
    onSave: () => void;
    onDiscard: () => void;
    onCancel: () => void;
}

// asked before the main window closes with unsaved changes
function UnsavedChangesModal({ onSave, onDiscard, onCancel }: UnsavedChangesModalProps) {
    // takes the keys while it's up: cancel, or confirm which saves unless a button is focused (that one's pressed)
    useModal(
        "dialog",
        {
            confirm: () => (document.activeElement instanceof HTMLButtonElement ? document.activeElement.click() : onSave()),
            cancel: onCancel,
        },
        true,
        { passthrough: true }
    );

    return (
        <div className="fixed inset-0 bg-black/20 flex items-center justify-center z-[9999] font-[Arial,sans-serif] select-none" onClick={onCancel}>
            <Glass className="glass-dialog w-80" onClick={(e) => e.stopPropagation()}>
                <div className="h-7 flex items-center px-3 pt-2 font-bold">Unsaved changes</div>
                <p className="px-3 py-3 text-sm">Save changes to this project before closing?</p>
                <div className="flex gap-2 justify-end px-3 pb-3">
                    <Button className="mr-auto" onClick={onDiscard}>
                        Don't Save
                    </Button>
                    <Button onClick={onCancel}>Cancel</Button>
                    <Button variant="filled" onClick={onSave}>
                        Save
                    </Button>
                </div>
            </Glass>
        </div>
    );
}

export default UnsavedChangesModal;
