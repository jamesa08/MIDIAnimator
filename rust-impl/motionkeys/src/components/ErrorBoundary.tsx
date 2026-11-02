import { Component, ErrorInfo, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

type ErrorBoundaryProps = {
    // what's inside, for the log and the fallback's header
    name: string;
    // drawn in place of the children once they've thrown, `retry` draws them again
    fallback?: (error: Error, retry: () => void) => ReactNode;
    // the children are drawn again when one of these changes (the state they were drawn from), they may not throw this time
    resetKeys?: unknown[];
    children?: ReactNode;
};

type ErrorBoundaryState = { error: Error | null; keys: unknown[] };

// keeps an error thrown while drawing one part of a window (a node, a panel, the graph) to that part, the rest of the
// window keeps working. react unmounts the whole window otherwise
class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
    state: ErrorBoundaryState = { error: null, keys: this.props.resetKeys ?? [] };

    static getDerivedStateFromError(error: unknown): Partial<ErrorBoundaryState> {
        return { error: error instanceof Error ? error : new Error(String(error)) };
    }

    // a reset key changed, try the children again
    static getDerivedStateFromProps(props: ErrorBoundaryProps, state: ErrorBoundaryState): Partial<ErrorBoundaryState> | null {
        const keys = props.resetKeys ?? [];
        const changed = keys.length !== state.keys.length || keys.some((key, i) => !Object.is(key, state.keys[i]));
        if (!changed) return null;
        return state.error ? { error: null, keys } : { keys };
    }

    // into the backend's log too, the console is gone in a release build. webkit's stack doesn't start with the message
    componentDidCatch(error: Error, info: ErrorInfo) {
        invoke("log", { message: `error in ${this.props.name}: ${error.message}\n${error.stack ?? ""}${info.componentStack ?? ""}` }).catch(() => {});
    }

    // in dev, an edit to the code (a hot update) draws the children again, the edit may have fixed what threw
    componentDidMount() {
        import.meta.hot?.on("vite:afterUpdate", this.retry);
    }

    componentWillUnmount() {
        import.meta.hot?.off("vite:afterUpdate", this.retry);
    }

    retry = () => {
        if (this.state.error) this.setState({ error: null });
    };

    render() {
        const { error } = this.state;
        if (!error) return this.props.children;
        if (this.props.fallback) return this.props.fallback(error, this.retry);
        return <ErrorPanel name={this.props.name} error={error} retry={this.retry} />;
    }
}

// the fallback for a part of a window: what threw and a button to draw it again
export function ErrorPanel({ name, error, retry }: { name: string; error: Error; retry: () => void }) {
    return (
        <div className="error-fallback w-full h-full flex flex-col font-[Arial,sans-serif] select-text">
            <div className="panel-header h-6 flex-none border-b border-black flex items-center pl-2 pr-2 text-sm">
                <span className="mr-auto">{name}</span>
                <button className="px-3 h-5 border border-black text-xs bg-white hover:bg-zinc-100" onClick={retry}>
                    Retry
                </button>
            </div>
            <pre className="flex-auto overflow-auto p-2 text-xs whitespace-pre-wrap">{error.message}</pre>
        </div>
    );
}

export default ErrorBoundary;
