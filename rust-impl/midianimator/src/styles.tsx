export const TEXT_SHADOW = "0 1px rgba(0,0,0,0.4)";
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

export const SOCKET_COLORS = {
    VALUE: "#a1a1a1",
    GEOMETRY: "#00daa0",
    VECTOR: "#6363ce",
    INT: "#488d57",
    BOOLEAN: "#d3a4d9",
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

// header color for a node with no category
export const DEFAULT_HEADER_COLOR = "#7A7A7A";

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
