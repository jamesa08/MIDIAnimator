function Panel({name}: {name: string}) {
    return (
        <div className="panel w-60 select-none">
            <div className="panel-header h-8 border-b border-black flex items-center pl-2 pr-2">{name}</div>
        </div>
    );
}

export default Panel;