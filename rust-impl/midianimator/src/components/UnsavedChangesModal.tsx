import { useEffect } from "react";

interface UnsavedChangesModalProps {
    onSave: () => void;
    onDiscard: () => void;
    onCancel: () => void;
}

// asked before the main window closes with unsaved changes
function UnsavedChangesModal({ onSave, onDiscard, onCancel }: UnsavedChangesModalProps) {
    // escape cancels, enter saves unless a button is focused
    useEffect(() => {
        const handleKey = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                e.stopPropagation();
                onCancel();
            } else if (e.key === "Enter" && !(document.activeElement instanceof HTMLButtonElement)) {
                e.stopPropagation();
                onSave();
            }
        };
        window.addEventListener("keydown", handleKey, true);
        return () => window.removeEventListener("keydown", handleKey, true);
    }, [onSave, onCancel]);

    const button = "px-3 h-6 border border-black text-sm";

    return (
        <div className="fixed inset-0 bg-black/20 flex items-center justify-center z-[9999] font-[Arial,sans-serif] select-none" onClick={onCancel}>
            <div className="bg-white border border-black w-80" onClick={(e) => e.stopPropagation()}>
                <div className="panel-header h-7 border-b border-black flex items-center pl-2 pr-2">Unsaved changes</div>
                <p className="px-2 py-3 text-sm">Save changes to this project before closing?</p>
                <div className="flex gap-2 justify-end px-2 pb-2">
                    <button className={`${button} mr-auto hover:bg-zinc-100`} onClick={onDiscard}>
                        Don't Save
                    </button>
                    <button className={`${button} hover:bg-zinc-100`} onClick={onCancel}>
                        Cancel
                    </button>
                    <button className={`${button} bg-black text-white hover:bg-zinc-800`} onClick={onSave}>
                        Save
                    </button>
                </div>
            </div>
        </div>
    );
}

export default UnsavedChangesModal;
