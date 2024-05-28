function Tab({ name }: { name: string }): JSX.Element {
    return (
        <div className="tab border border-black w-20 md:w-30 lg:w-40 grow lg:grow-0 p-2 border-b-0 border-t-0 flex items-center select-none -ml-[1px]" title={`Tree: ${name}\nNodes: 100`}>
            <div className="name whitespace-nowrap text-ellipsis overflow-hidden w-4/5">{name}</div>
            <div className="close ml-auto">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor" className="size-6">
                    <path strokeLinecap="round" strokeLinejoin="round" d="M6 18 18 6M6 6l12 12" />
                </svg>
            </div>
        </div>
    );
}

export default Tab;
