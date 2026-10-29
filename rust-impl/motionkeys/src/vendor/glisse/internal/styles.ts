const id = "glisse-styles";

/**
 * Adds the stylesheet to the document once. It goes first in <head>, so the
 * app's own CSS comes after it and wins ties, as it would if imported after.
 */
export function insertStyles(css: string): void {
  if (typeof document === "undefined" || document.getElementById(id)) return;
  const style = document.createElement("style");
  style.id = id;
  style.textContent = css;
  document.head.prepend(style);
}
