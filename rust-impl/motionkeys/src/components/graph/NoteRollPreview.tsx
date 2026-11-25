import { useCallback } from "react";
import { ThemeColor } from "../../utils/curveDrawing";
import { HEADER_COLORS } from "../../styles";
import { PreviewBox, usePreviewCanvas } from "./previewCanvas";

type Note = { note_number: number; time_on: number; time_off: number };

// the black keys in an octave, their rows are shaded like a piano roll's
const BLACK_KEYS = new Set([1, 3, 6, 8, 10]);

// a piano roll of a track's notes (`notes`, Get MIDI Track Data's): a row for every note number from the lowest to the
// highest, time from 0 to the last note's end
function NoteRollPreview({ notes, height = 80 }: { notes: any; height?: number }) {
    const paint = useCallback(
        (ctx: CanvasRenderingContext2D, width: number, height: number, color: ThemeColor) => {
            ctx.fillStyle = color("--graph-background");
            ctx.fillRect(0, 0, width, height);
            const list: Note[] = Array.isArray(notes) ? notes.filter((n: any) => typeof n?.note_number === "number" && typeof n?.time_on === "number") : [];
            if (list.length === 0) return;
            let [low, high, end] = [Infinity, -Infinity, 0];
            for (const note of list) {
                low = Math.min(low, note.note_number);
                high = Math.max(high, note.note_number);
                end = Math.max(end, note.time_off ?? note.time_on);
            }
            const rows = high - low + 1;
            const row = height / rows;
            const x = (time: number) => (time / (end || 1)) * width;
            ctx.fillStyle = color("--graph-strip");
            for (let number = low; number <= high; number++) if (BLACK_KEYS.has(number % 12)) ctx.fillRect(0, (high - number) * row, width, row);
            ctx.fillStyle = HEADER_COLORS.midi;
            for (const note of list) {
                const left = x(note.time_on);
                ctx.fillRect(left, (high - note.note_number) * row + row * 0.1, Math.max(1, x(note.time_off ?? note.time_on) - left - 0.5), Math.max(1, row * 0.8));
            }
        },
        [notes]
    );
    const canvasRef = usePreviewCanvas(paint);
    return <PreviewBox canvasRef={canvasRef} height={height} />;
}

export default NoteRollPreview;
