// live window resize (src-tauri/src/ui/windows.rs). webkit repaints the whole page whenever the size it's laid out at
// changes, so for a window drag the page is laid out at a fixed size (the screen's) and the native side calls size()
// every step to fit the app to the window instead, inline on just what follows the window so nothing else restyles.
// end() runs as the page goes back to being laid out at the window's size.
// data-live-resize="window" marks a page's root sized with viewport units, "canvas" the node graph

declare global {
    interface Window {
        liveResize?: { size: (width: number, height: number, layoutWidth: number, layoutHeight: number) => void; end: () => void };
    }
}

const each = (selector: string, apply: (element: HTMLElement) => void) => document.querySelectorAll<HTMLElement>(selector).forEach(apply);

window.liveResize = {
    size(width, height, layoutWidth, layoutHeight) {
        const root = document.documentElement;
        root.classList.add("live-resize");
        root.style.width = `${width}px`;
        root.style.height = `${height}px`;
        each('[data-live-resize="window"]', (element) => {
            element.style.width = `${width}px`;
            element.style.height = `${height}px`;
        });

        // the canvas keeps one size, as big as its card can get (the card's size plus what the window can still grow by,
        // which works out the same every step), so it's never repainted. its controls on the right and bottom edges are
        // moved back onto the card's edges by what the canvas overhangs them
        const overhangX = layoutWidth - width;
        const overhangY = layoutHeight - height;
        each('[data-live-resize="canvas"]', (element) => {
            element.style.width = `calc(100% + ${overhangX}px)`;
            element.style.height = `calc(100% + ${overhangY}px)`;
        });
        each('[data-live-resize="canvas"] .react-flow__panel:not(.center)', (element) => {
            const x = element.classList.contains("right") ? -overhangX : 0;
            const y = element.classList.contains("bottom") ? -overhangY : 0;
            if (x || y) element.style.transform = `translate(${x}px, ${y}px)`;
        });
    },

    end() {
        const root = document.documentElement;
        root.classList.remove("live-resize");
        for (const element of [root, ...document.querySelectorAll<HTMLElement>("[data-live-resize]")]) {
            element.style.width = "";
            element.style.height = "";
        }
        each('[data-live-resize="canvas"] .react-flow__panel', (element) => (element.style.transform = ""));
    },
};

export {};
