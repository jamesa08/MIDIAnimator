/**
 * How long a touch rests on a control before it belongs to the control rather
 * than the page, as with UIScrollView's delay before its controls take a
 * touch. A quick swipe that starts on a control still scrolls the page; a
 * press held this long, or a drag already under way, keeps the touch.
 */
export const HOLD = 150;

export const held = (at: number, moved: boolean): boolean => moved || performance.now() - at >= HOLD;
