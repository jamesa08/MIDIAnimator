import { useRef } from "react";
import { createPortal } from "react-dom";
import { useModal } from "../utils/keymap";
import { Button, Glass } from "glisse";

interface SceneDiff {
    missing_objects: string[];
    new_objects: string[];
    missing_collections: string[];
    new_collections: string[];
}

interface SceneDiffModalProps {
    diff: SceneDiff | null;
    onAccept: () => void;
    onReject: () => void;
    onClose: () => void;
}

function SceneDiffModal({ diff, onAccept, onReject, onClose }: SceneDiffModalProps) {
    const modalRef = useRef<HTMLDivElement>(null);

    // takes the keys while it's up, cancel closes it
    useModal("dialog", { cancel: onClose }, !!diff, { passthrough: true });

    if (!diff) return null;

    const hasChanges = diff.missing_objects.length > 0 || diff.new_objects.length > 0 || diff.missing_collections.length > 0 || diff.new_collections.length > 0;

    // on the body so it sits above everything, the tab strip it's opened from is its own stacking context
    return createPortal(
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-[9999]" onClick={onClose}>
            <Glass ref={modalRef} className="glass-dialog p-6 max-w-2xl max-h-[80vh] overflow-auto" onClick={(e) => e.stopPropagation()}>
                <h2 className="text-xl font-bold mb-4">Scene Data Validation</h2>

                {!hasChanges ? (
                    <div className="mb-4">
                        <p className="text-green-600">✓ No changes detected. Scene data matches.</p>
                    </div>
                ) : (
                    <>
                        <p className="mb-4 text-gray-700">The Blender scene has changed since the project was saved. Review the changes below:</p>

                        {diff.missing_objects.length > 0 && (
                            <div className="mb-4">
                                <h3 className="font-semibold text-red-600 mb-2">Missing Objects ({diff.missing_objects.length}):</h3>
                                <ul className="list-disc list-inside ml-4 text-sm max-h-40 overflow-auto">
                                    {diff.missing_objects.map((obj, idx) => (
                                        <li key={idx} className="text-gray-700">
                                            {obj}
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}

                        {diff.new_objects.length > 0 && (
                            <div className="mb-4">
                                <h3 className="font-semibold text-green-600 mb-2">New Objects ({diff.new_objects.length}):</h3>
                                <ul className="list-disc list-inside ml-4 text-sm max-h-40 overflow-auto">
                                    {diff.new_objects.map((obj, idx) => (
                                        <li key={idx} className="text-gray-700">
                                            {obj}
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}

                        {diff.missing_collections.length > 0 && (
                            <div className="mb-4">
                                <h3 className="font-semibold text-red-600 mb-2">Missing Collections ({diff.missing_collections.length}):</h3>
                                <ul className="list-disc list-inside ml-4 text-sm max-h-40 overflow-auto">
                                    {diff.missing_collections.map((coll, idx) => (
                                        <li key={idx} className="text-gray-700">
                                            {coll}
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}

                        {diff.new_collections.length > 0 && (
                            <div className="mb-4">
                                <h3 className="font-semibold text-green-600 mb-2">New Collections ({diff.new_collections.length}):</h3>
                                <ul className="list-disc list-inside ml-4 text-sm max-h-40 overflow-auto">
                                    {diff.new_collections.map((coll, idx) => (
                                        <li key={idx} className="text-gray-700">
                                            {coll}
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}
                    </>
                )}

                <div className="flex gap-4 justify-end mt-6">
                    {hasChanges ? (
                        <>
                            <Button onClick={onReject}>Reject Changes</Button>
                            <Button variant="filled" onClick={onAccept}>
                                Accept & Update Scene Data
                            </Button>
                        </>
                    ) : (
                        <Button variant="filled" onClick={onAccept}>
                            Continue
                        </Button>
                    )}
                </div>
            </Glass>
        </div>,
        document.body
    );
}

export default SceneDiffModal;
