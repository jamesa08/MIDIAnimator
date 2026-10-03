// live window resize (src-tauri/src/ui/windows.rs). webkit repaints the whole page whenever the size it's laid out at
// changes, so for a window drag the page is laid out at a fixed size (the screen's) and the native side calls size()
// every step to fit the app to the window instead, inline on just what follows the window so nothing else restyles.
// end() runs as the page goes back to being laid out at the window's size.
// data-live-resize="window" marks a page's root sized with viewport units, "vignette" the canvas' vignette. the
// canvas' other costly parts keep one size for the drag on their own (index.css .live-resize)

declare global {
    interface Window {
        liveResize?: { size: (width: number, height: number, layoutWidth: number, layoutHeight: number) => void; end: () => void };
    }
}

const sized = () => [document.documentElement, ...document.querySelectorAll<HTMLElement>('[data-live-resize="window"]')];
const vignettes = () => document.querySelectorAll<HTMLElement>('[data-live-resize="vignette"]');

// how much smaller than the window the vignette's card is, measured once a drag
let cardInset: { width: number; height: number } | null = null;

window.liveResize = {
    size(width, height, layoutWidth, layoutHeight) {
        document.documentElement.classList.add("live-resize");
        for (const element of sized()) {
            element.style.width = `${width}px`;
            element.style.height = `${height}px`;
        }

        // the vignette is laid out at the page's size for the drag and scaled down to its card, so it's never repainted
        for (const element of vignettes()) {
            const card = element.parentElement;
            if (!card) continue;
            cardInset ??= { width: width - card.clientWidth, height: height - card.clientHeight };
            const scaleX = (width - cardInset.width) / layoutWidth;
            const scaleY = (height - cardInset.height) / layoutHeight;
            element.style.transform = `translate(-50%, -50%) scale(${scaleX}, ${scaleY})`;
        }
    },

    end() {
        document.documentElement.classList.remove("live-resize");
        for (const element of sized()) {
            element.style.width = "";
            element.style.height = "";
        }
        for (const element of vignettes()) element.style.transform = "";
        cardInset = null;
    },
};

export {};
