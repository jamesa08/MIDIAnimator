import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { HEADER_COLORS, SOCKET_COLORS } from "../../styles";
import { noteMapData, shownMap } from "../../utils/noteMap";

// how wide the notes' bars are (the objects' take a share of the preview for their names), and the smallest their text is
// drawn (rows too thin for it get none)
const BAR = 26;
const OBJECT_SHARE = 0.38;
const INSET = 4;
const MIN_TEXT = 6;
const TEXT_PAD = 3;

// a name cut short with … to fit `width` at `size`, measured the way it's drawn
let measure: CanvasRenderingContext2D | null = null;
function fitText(text: string, width: number, size: number): string {
    measure ??= document.createElement("canvas").getContext("2d");
    if (!measure) return text;
    measure.font = `${size}px Arial, sans-serif`;
    if (measure.measureText(text).width <= width) return text;
    let end = text.length;
    while (end > 0 && measure.measureText(text.slice(0, end) + "…").width > width) end--;
    return end > 0 ? text.slice(0, end) + "…" : "";
}

// a small picture of Assign Notes to Objects' note map on the node, like the curve previews: its notes on the left with
// their numbers, its objects on the right with their names and a line for each note an object gets. the same notes and
// wires the map shows (Tab), in its default columns, squeezed to fit
function NoteMapPreview({ inputs, results, nodeInputs, height = 80 }: { inputs: any; results: any; nodeInputs: any; height?: number }) {
    const shown = useMemo(() => {
        const info = noteMapData(inputs, results, nodeInputs?.object_group_name);
        const { map, notes } = shownMap(nodeInputs?.mode ?? "rules", info, nodeInputs?.note_map);
        return { info, map, notes };
    }, [inputs, results, nodeInputs?.object_group_name, nodeInputs?.mode, nodeInputs?.note_map]);
    const { info, map, notes } = shown;
    const objects = info.objects;

    // drawn in the preview's own pixels so the numbers aren't stretched, its width follows the node's
    const boxRef = useRef<HTMLDivElement>(null);
    const [width, setWidth] = useState(0);
    useLayoutEffect(() => {
        const box = boxRef.current;
        if (!box) return;
        const resize = new ResizeObserver(() => setWidth(box.clientWidth));
        resize.observe(box);
        setWidth(box.clientWidth);
        return () => resize.disconnect();
    }, []);
    const inner = height - 2;

    const noteRow = inner / Math.max(notes.length, 1);
    const objectRow = inner / Math.max(objects.length, 1);
    const noteY = (i: number) => (i + 0.5) * noteRow;
    const objectY = (i: number) => (i + 0.5) * objectRow;
    const noteRight = INSET + BAR;
    const objectBar = Math.max(BAR, Math.round(width * OBJECT_SHARE));
    const objectLeft = width - INSET - objectBar;
    const middle = (noteRight + objectLeft) / 2;
    const textSize = Math.min(9, noteRow * 0.75);
    const objectTextSize = Math.min(9, objectRow * 0.75);

    return (
        <div ref={boxRef} className="node-field relative border border-[var(--chrome-line)]" style={{ height, background: "var(--graph-background)" }}>
            {width > 0 && (
                <svg className="absolute inset-0 block" width={width} height={inner}>
                    {objects.flatMap((o, oi) =>
                        (map.objects[o] ?? []).map((n) => {
                            const y1 = noteY(notes.indexOf(n));
                            const y2 = objectY(oi);
                            return <path key={`${n}>${o}`} d={`M ${noteRight} ${y1} C ${middle} ${y1}, ${middle} ${y2}, ${objectLeft} ${y2}`} fill="none" stroke={SOCKET_COLORS.midi} strokeWidth={1.5} />;
                        })
                    )}
                    {/* notes the MIDI doesn't play are paler, like in the map */}
                    {notes.map((n, i) => (
                        <g key={n} opacity={info.midiNotes.includes(n) ? 1 : 0.45}>
                            <rect x={INSET} y={i * noteRow + noteRow * 0.15} width={BAR} height={noteRow * 0.7} fill={HEADER_COLORS.midi} />
                            {textSize >= MIN_TEXT && (
                                <text x={INSET + BAR / 2} y={noteY(i)} fontSize={textSize} fill="#fff" textAnchor="middle" dominantBaseline="central" fontFamily="Arial, sans-serif">
                                    {n}
                                </text>
                            )}
                        </g>
                    ))}
                    {objects.map((o, i) => (
                        <g key={o}>
                            <rect x={objectLeft} y={i * objectRow + objectRow * 0.15} width={objectBar} height={objectRow * 0.7} fill={HEADER_COLORS.scene} />
                            {objectTextSize >= MIN_TEXT && (
                                <text x={objectLeft + TEXT_PAD} y={objectY(i)} fontSize={objectTextSize} fill="#fff" dominantBaseline="central" fontFamily="Arial, sans-serif">
                                    {fitText(o, objectBar - TEXT_PAD * 2, objectTextSize)}
                                </text>
                            )}
                        </g>
                    ))}
                </svg>
            )}
        </div>
    );
}

export default NoteMapPreview;
