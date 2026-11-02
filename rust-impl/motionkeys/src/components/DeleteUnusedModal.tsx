import { useModal } from "../utils/keymap";

interface DeleteUnusedModalProps {
    count: number;
    onDelete: () => void;
    onCancel: () => void;
}

// asked before Delete Unused Nodes deletes `count` nodes
function DeleteUnusedModal({ count, onDelete, onCancel }: DeleteUnusedModalProps) {
    // takes the keys while it's up: cancel, or confirm which deletes unless a button is focused (that one's pressed)
    useModal(
        "dialog",
        {
            confirm: () => (document.activeElement instanceof HTMLButtonElement ? document.activeElement.click() : onDelete()),
            cancel: onCancel,
        },
        true,
        { passthrough: true }
    );

    const button = "px-3 h-6 border border-black text-sm";

    return (
        <div className="fixed inset-0 bg-black/20 flex items-center justify-center z-[9999] font-[Arial,sans-serif] select-none" onClick={onCancel}>
            <div className="bg-white border border-black w-80" onClick={(e) => e.stopPropagation()}>
                <div className="panel-header h-7 border-b border-black flex items-center pl-2 pr-2">Delete Unused Nodes</div>
                <p className="px-2 py-3 text-sm">{count === 1 ? "Delete 1 unused node?" : `Delete ${count} unused nodes?`}</p>
                <div className="flex gap-2 justify-end px-2 pb-2">
                    <button className={`${button} hover:bg-zinc-100`} onClick={onCancel}>
                        Cancel
                    </button>
                    <button className={`${button} bg-black text-white hover:bg-zinc-800`} onClick={onDelete}>
                        Delete
                    </button>
                </div>
            </div>
        </div>
    );
}

export default DeleteUnusedModal;
