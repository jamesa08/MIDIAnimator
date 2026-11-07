import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// a compile error from check_expression, start..end in characters
export type ExprError = { kind: string; message: string; start: number; end: number };

// a mapper's text as the node saves it. one saved with the expression inside an object (how mappers were saved for a
// while) gives its expression, like the backend reads it
export const mapperText = (value: any): string => (typeof value === "string" ? value : typeof value?.expression === "string" ? value.expression : "");

// the variables mapper expressions can use, from the backend
let mapperVariables: Promise<string[]> | null = null;
export function useMapperVariables(): string[] {
    const [vars, setVars] = useState<string[]>([]);
    useEffect(() => {
        mapperVariables ??= invoke<string[]>("mapper_variables").catch(() => []);
        let live = true;
        mapperVariables.then((v) => live && setVars(v ?? []));
        return () => {
            live = false;
        };
    }, []);
    return vars;
}
