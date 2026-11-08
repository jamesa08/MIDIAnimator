import { useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { HEADER_COLORS, SOCKET_COLORS } from "../../styles";
import { MapView, isAnimationLinked, isLinked, noteMapData, shownAnimations, shownMap } from "../../utils/noteMap";

// how wide the notes' bars are (the objects' and animations' take a share of the preview for their names), and the
// smallest their text is drawn (rows too thin for it get none)
const BAR = 26;
const OBJECT_SHARE = 0.38;
const ANIMATION_SHARE = 0.22;
// where an animation's wires end, its own color like the map's (solid over the fade to the object's color)
const ANIMATION_WIRE_END = SOCKET_COLORS.generator;
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
// their numbers and its animations under them, its objects on the right with their names and a line for each note and
// animation an object gets. the same nodes and wires the map shows (Tab) in its view, in its default columns, squeezed
// to fit. with linked only, only the notes and animations wired to an object
function NoteMapPreview({ inputs, results, nodeInputs, linkedOnly = false, view = "combined", height = 80 }: { inputs: any; results: any; nodeInputs: any; linkedOnly?: boolean; view?: MapView; height?: number }) {
    const shown = useMemo(() => {
        const info = noteMapData(inputs, results, nodeInputs?.object_group_name);
        const { map, notes } = shownMap(nodeInputs?.mode ?? "rules", info, nodeInputs?.note_map);
        const shownNotes = view === "animations" ? [] : linkedOnly ? notes.filter((n) => isLinked(map, info.objects, n)) : notes;
        const shownAnimationNames = view === "notes" ? [] : shownAnimations(map, info).filter((a) => !linkedOnly || isAnimationLinked(map, info.objects, a));
        return { info, map, animations: shownAnimationNames, notes: shownNotes };
    }, [inputs, results, nodeInputs?.object_group_name, nodeInputs?.mode, nodeInputs?.note_map, linkedOnly, view]);
    const { info, map, animations, notes } = shown;
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

    // the animations take rows under the notes, in the same column
    const noteRow = inner / Math.max(notes.length + animations.length, 1);
    const objectRow = inner / Math.max(objects.length, 1);
    const noteY = (i: number) => (i + 0.5) * noteRow;
    const animationY = (i: number) => noteY(notes.length + i);
    const objectY = (i: number) => (i + 0.5) * objectRow;
    const noteRight = INSET + BAR;
    const objectBar = Math.max(BAR, Math.round(width * OBJECT_SHARE));
    const objectLeft = width - INSET - objectBar;
    const animationBar = Math.max(BAR, Math.round(width * ANIMATION_SHARE));
    const animationRight = INSET + animationBar;
    const textSize = Math.min(9, noteRow * 0.75);
    const objectTextSize = Math.min(9, objectRow * 0.75);
    // a wire from one bar's right to another's left, like the map's (react flow's bezier). it fades from the color it
    // starts in to the one it ends in, like the map's wires
    const gradientBase = `note-map-preview-${useId()}`;
    let gradients = 0;
    const wire = (key: string, x1: number, y1: number, x2: number, y2: number, from = SOCKET_COLORS.midi, to = SOCKET_COLORS.midi) => {
        const dx = x2 - x1;
        const bend = dx >= 0 ? dx / 2 : 0.25 * 25 * Math.sqrt(-dx);
        const d = `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`;
        if (from === to) return <path key={key} d={d} fill="none" stroke={from} strokeWidth={1.5} />;
        const id = `${gradientBase}-${gradients++}`;
        return (
            <g key={key}>
                <linearGradient id={id} gradientUnits="userSpaceOnUse" x1={x1} y1={y1} x2={x2} y2={y2}>
                    <stop offset="0%" style={{ stopColor: from }} />
                    <stop offset="100%" style={{ stopColor: to }} />
                </linearGradient>
                <path d={d} fill="none" stroke={`url(#${id})`} strokeWidth={1.5} />
            </g>
        );
    };

    return (
        <div ref={boxRef} className="node-field relative border border-[var(--chrome-line)]" style={{ height, background: "var(--graph-background)" }}>
            {width > 0 && (
                <svg className="absolute inset-0 block" width={width} height={inner}>
                    {objects.flatMap((o, oi) => (map.objects[o] ?? []).filter((n) => notes.includes(n)).map((n) => wire(`${n}>${o}`, noteRight, noteY(notes.indexOf(n)), objectLeft, objectY(oi))))}
                    {animations.flatMap((a, ai) => (map.animations[a] ?? []).filter((o) => objects.includes(o)).map((o) => wire(`a:${a}>${o}`, animationRight, animationY(ai), objectLeft, objectY(objects.indexOf(o)), SOCKET_COLORS.generator, ANIMATION_WIRE_END)))}
                    {/* animations whose generator isn't connected are paler, like in the map */}
                    {animations.map((a, i) => (
                        <g key={a} opacity={info.animations.includes(a) ? 1 : 0.45}>
                            <rect x={INSET} y={(notes.length + i) * noteRow + noteRow * 0.15} width={animationBar} height={noteRow * 0.7} fill={HEADER_COLORS.animation} />
                            {textSize >= MIN_TEXT && (
                                <text x={INSET + TEXT_PAD} y={animationY(i)} fontSize={textSize} fill="#fff" dominantBaseline="central" fontFamily="Arial, sans-serif">
                                    {fitText(a, animationBar - TEXT_PAD * 2, textSize)}
                                </text>
                            )}
                        </g>
                    ))}
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
                                    {fitText(info.objectNames[o] ?? o, objectBar - TEXT_PAD * 2, objectTextSize)}
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
