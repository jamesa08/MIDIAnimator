import { useEffect, useRef } from "react";

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

    // Close on Escape key
    useEffect(() => {
        const handleEscape = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                onClose();
            }
        };
        window.addEventListener("keydown", handleEscape);
        return () => window.removeEventListener("keydown", handleEscape);
    }, [onClose]);

    if (!diff) return null;

    const hasChanges = diff.missing_objects.length > 0 || diff.new_objects.length > 0 || diff.missing_collections.length > 0 || diff.new_collections.length > 0;

    return (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-[9999]" onClick={onClose}>
            <div ref={modalRef} className="bg-white rounded-lg p-6 max-w-2xl max-h-[80vh] overflow-auto" onClick={(e) => e.stopPropagation()}>
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
                            <button className="px-4 py-2 bg-gray-300 text-black rounded hover:bg-gray-400" onClick={onReject}>
                                Reject Changes
                            </button>
                            <button className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700" onClick={onAccept}>
                                Accept & Update Scene Data
                            </button>
                        </>
                    ) : (
                        <button className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700" onClick={onAccept}>
                            Continue
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}

export default SceneDiffModal;
