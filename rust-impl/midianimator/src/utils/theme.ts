// themes are css files in src/themes, loaded on top of index.css. "light" is index.css on its own

// every theme file as its css text, loaded only once it's picked
const THEME_FILES = import.meta.glob("../themes/*.css", { query: "?inline", import: "default" }) as Record<string, () => Promise<string>>;

// theme ids from the file names, e.g. "../themes/blueprint.css" is "blueprint"
export const THEMES = ["light", ...Object.keys(THEME_FILES).map((path) => path.slice(path.lastIndexOf("/") + 1, -".css".length))];

// the style element the current theme's css goes in
let themeStyle: HTMLStyleElement | null = null;
// the theme last asked for, so a slow load can't override a newer pick
let requested = "light";

// swaps the page's theme css for the given theme's, no reload needed
export async function applyTheme(theme: string) {
    requested = theme;
    const load = THEME_FILES[`../themes/${theme}.css`];
    const css = load ? await load() : "";
    if (requested !== theme) return;

    // appended to the end of head so it wins over index.css and react flow's styles
    if (!themeStyle) {
        themeStyle = document.createElement("style");
        themeStyle.id = "theme";
    }
    themeStyle.textContent = css;
    document.head.appendChild(themeStyle);
}
