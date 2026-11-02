import { Dispatch, SetStateAction, useCallback, useState } from "react";

// ui state kept across reloads of a window (a dev reload, cmd+r) but not across launches. the backend has the project, the
// layout and the open group already, this is what only the front end knows. sessionStorage is the window's own and goes
// when the app quits. it can be missing or throw, then the state just starts over
const PREFIX = "motionkeys.";

// what's kept under `key`, `fallback` when there's nothing or it can't be read
export function loadPersisted<T>(key: string, fallback: T): T {
    try {
        const raw = sessionStorage.getItem(PREFIX + key);
        return raw === null ? fallback : JSON.parse(raw);
    } catch {
        return fallback;
    }
}

// keeps `value` under `key`, undefined removes it
export function savePersisted(key: string, value: unknown) {
    try {
        if (value === undefined) sessionStorage.removeItem(PREFIX + key);
        else sessionStorage.setItem(PREFIX + key, JSON.stringify(value));
    } catch {}
}

// useState kept under `key`, it starts where it was before the reload. `codec` turns values that aren't plain JSON (a Set)
// into JSON and back
export function usePersistedState<T>(key: string, initial: T | (() => T), codec?: { encode: (value: T) => unknown; decode: (raw: any) => T }): [T, Dispatch<SetStateAction<T>>] {
    const [value, setValue] = useState<T>(() => {
        const fallback = typeof initial === "function" ? (initial as () => T)() : initial;
        const raw = loadPersisted<unknown>(key, undefined);
        if (raw === undefined) return fallback;
        try {
            return codec ? codec.decode(raw) : (raw as T);
        } catch {
            return fallback;
        }
    });

    // saved as it's set, so a reload right after still has it
    const set = useCallback(
        (update: SetStateAction<T>) =>
            setValue((prev) => {
                const next = typeof update === "function" ? (update as (prev: T) => T)(prev) : update;
                savePersisted(key, codec ? codec.encode(next) : next);
                return next;
            }),
        [key]
    );
    return [value, set];
}

const SET_CODEC = { encode: (value: Set<string>) => [...value], decode: (raw: any) => new Set<string>(raw) };

// a persisted Set of strings, see usePersistedState
export function usePersistedSet(key: string, initial: string[] = []) {
    return usePersistedState(key, () => new Set(initial), SET_CODEC);
}
