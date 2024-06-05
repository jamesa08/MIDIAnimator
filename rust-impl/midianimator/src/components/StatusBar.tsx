function StatusBar({ event }: { event: string }) {
    return (
        <div className="status-bar w-screen border-black border select-none">
            <div className="panel-header text-xs p-0.3 border-b border-black flex items-center pl-2 pr-2">{event}</div>
        </div>
    );
}

export default StatusBar;
