function Tab({ name }: { name: string }): JSX.Element {
    return (
        <div className="tab border border-black min-w-10 max-w-60 flex-1 p-2 border-b-0 border-t-0 flex items-center select-none first:ml-[-1px] -ml-[1px] text-sm overflow-hidden" title={`Tree: ${name}\nNodes: 100`}>
            <div className="name whitespace-nowrap text-ellipsis overflow-hidden flex-1 min-w-0 mr-4 cursor-default">{name}</div>
            <div className="close shrink-0 ml-auto cursor-pointer">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-4">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
                </svg>
            </div>
        </div>
    );
}

export default Tab;
