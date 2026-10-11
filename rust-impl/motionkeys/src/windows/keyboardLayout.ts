import { Modifier } from "../utils/keymap";

// one key drawn on the shortcut editor's keyboard, in key widths from the top left. `name` is the key it binds (as the keymap
// writes it), null for a key nothing can be bound to
export type KeyCap = { name: string | null; label: string; x: number; y: number; w: number; h: number; modifier?: Modifier };

// the full mac keyboard (with the numeric keypad), 23 keys wide and 6 tall
export const KEYBOARD_WIDTH = 23;
export const KEYBOARD_HEIGHT = 5.75;

// a row of keys from `x`, each [name, label, width]
function row(y: number, x: number, keys: [string | null, string, number?][], h = 1): KeyCap[] {
    return keys.map(([name, label, w = 1]) => {
        const cap: KeyCap = { name, label, x, y, w, h };
        x += w;
        return cap;
    });
}

const letters = (names: string) => names.split("").map((n): [string, string] => [n, n]);
const MAIN = 0;
const NAV = 15.5;
const PAD = 19;
const FN_ROW = 0.75;

export function keyboardLayout(mac: boolean): KeyCap[] {
    const control = mac ? "control" : "ctrl";
    const option = mac ? "option" : "alt";
    const command = mac ? "command" : "win";
    const top = (n: number) => FN_ROW + n;
    const caps: KeyCap[] = [
        // function row
        ...row(0, MAIN, [["Escape", "esc", 1.5], ...Array.from({ length: 12 }, (_, i): [string, string, number] => [`F${i + 1}`, `F${i + 1}`, 13.5 / 12])], FN_ROW),
        ...row(
            0,
            NAV,
            [
                ["F13", "F13"],
                ["F14", "F14"],
                ["F15", "F15"],
            ],
            FN_ROW
        ),
        ...row(
            0,
            PAD,
            [
                ["F16", "F16"],
                ["F17", "F17"],
                ["F18", "F18"],
                ["F19", "F19"],
            ],
            FN_ROW
        ),
        // main block
        ...row(top(0), MAIN, [["`", "`"], ...letters("1234567890"), ["-", "-"], ["=", "="], ["Backspace", mac ? "delete" : "backspace", 2]]),
        ...row(top(1), MAIN, [["Tab", "tab", 1.5], ...letters("QWERTYUIOP"), ["[", "["], ["]", "]"], ["\\", "\\", 1.5]]),
        ...row(top(2), MAIN, [[null, "caps lock", 1.75], ...letters("ASDFGHJKL"), [";", ";"], ["'", "'"], ["Enter", mac ? "return" : "enter", 2.25]]),
        ...row(top(3), MAIN, [["Shift", "shift", 2.25], ...letters("ZXCVBNM"), [",", ","], [".", "."], ["/", "/"], ["Shift", "shift", 2.75]]),
        ...row(top(4), MAIN, [
            ["Ctrl", control, 1.75],
            ["Alt", option, 1.5],
            ["Cmd", command, 1.75],
            ["Space", "", 6.25],
            ["Cmd", command, 1.75],
            ["Alt", option, 2],
        ]),
        // navigation
        ...row(top(0), NAV, [
            [null, "fn"],
            ["Home", "home"],
            ["PageUp", "page up"],
        ]),
        ...row(top(1), NAV, [
            ["Delete", mac ? "⌦" : "delete"],
            ["End", "end"],
            ["PageDown", "page down"],
        ]),
        ...row(top(3), NAV + 1, [["Up", "↑"]]),
        ...row(top(4), NAV, [
            ["Left", "←"],
            ["Down", "↓"],
            ["Right", "→"],
        ]),
        // numeric keypad
        ...row(top(0), PAD, [
            ["NumClear", "clear"],
            ["NumEqual", "="],
            ["NumDivide", "/"],
            ["NumMultiply", "*"],
        ]),
        ...row(top(1), PAD, [
            ["Num7", "7"],
            ["Num8", "8"],
            ["Num9", "9"],
            ["NumSubtract", "-"],
        ]),
        ...row(top(2), PAD, [
            ["Num4", "4"],
            ["Num5", "5"],
            ["Num6", "6"],
            ["NumAdd", "+"],
        ]),
        ...row(top(3), PAD, [
            ["Num1", "1"],
            ["Num2", "2"],
            ["Num3", "3"],
        ]),
        ...row(top(3), PAD + 3, [["NumEnter", "enter"]], 2),
        ...row(top(4), PAD, [
            ["Num0", "0", 2],
            ["NumDecimal", "."],
        ]),
    ];
    for (const cap of caps) {
        if (cap.name === "Shift" || cap.name === "Ctrl" || cap.name === "Alt" || cap.name === "Cmd") cap.modifier = cap.name;
    }
    return caps;
}
