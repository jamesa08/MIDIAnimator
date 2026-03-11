function AddTabButton(): JSX.Element {
    return (
        <div className="add-tab-button flex-1 p-2 flex items-center select-none cursor-pointer max-min" title="Add new tab">
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="lightgray" className="size-4">
                <path strokeLinecap="round" strokeLinejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
            </svg>
        </div>
    );
}

export default AddTabButton;
