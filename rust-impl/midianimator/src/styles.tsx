export const INPUT_BORDER_RADIUS = 4;
export const CHECKBOX_BORDER_RADIUS = 3;
export const ACTIVE_BUTTON_BG = "#3873b8";
export const INPUT_BG = "#545555";

export const COLORS = {
    purple: "#8e294b",
    blue: "#006487",
    green: "#00745e",
    black: "#1d1d1d",
    darkPurple: "#411b26",
    lightPurple: "#3c3c88",
};

// socket color per value category (see utils/sockets.ts), in the same families as the header colors.
// number and string are css vars (index.css) so themes can change them
export const SOCKET_COLORS = {
    midi: "#D4AE3A",
    scene: "#4DB887",
    generator: "#D96A78",
    object_map: "#A8434F",
    target: "#E89AA5",
    keyframes: "#C2577F",
    number: "var(--socket-number)",
    string: "var(--socket-string)",
    any: "#A1A1A1",
};

// header color per node category (the spec's `category`), soft mid tones under white text.
// a built-in group gets its category's `_group` color, the project's own groups `group`
export const HEADER_COLORS = {
    midi: "#B8962E",
    scene: "#3E9E72",
    animation: "#C95B6A",
    animation_group: "#A8434F",
    viewer: "#8A6BBE",
    group: "#6FA83E",
    interface: "#5A5A5A",
    zone: "#6674C4",
};

// node border per category, the header's hue but darker and more saturated. selected nodes glow in it
export const BORDER_COLORS = {
    midi: "#9A7408",
    scene: "#1F8055",
    animation: "#B02A3E",
    animation_group: "#8E1C2A",
    viewer: "#6437B0",
    group: "#4E8A17",
    interface: "#3A3A3A",
    zone: "#3445B8",
};

// colors for a node with no category
export const DEFAULT_HEADER_COLOR = "#7A7A7A";
export const DEFAULT_BORDER_COLOR = "#555555";

// css vars the node and its header are drawn with (index.css)
export function nodeColors(category: string | undefined): Record<string, string> {
    return {
        "--node-header": HEADER_COLORS[category as keyof typeof HEADER_COLORS] ?? DEFAULT_HEADER_COLOR,
        "--node-border": BORDER_COLORS[category as keyof typeof BORDER_COLORS] ?? DEFAULT_BORDER_COLOR,
    };
}

// order categories are listed in (add menu, nodes panel)
export const CATEGORY_ORDER = ["midi", "scene", "animation", "viewer", "zone", "group"];

export const SOCKET_SHAPES = {
    CIRCLE: {},
    DIAMOND: {
        borderRadius: 0,
        transform: "rotate(45deg)",
        top: 8,
    },
    DIAMOND_DOT: {
        borderRadius: 0,
        transform: "rotate(45deg)",
        top: 8,
    },
};
