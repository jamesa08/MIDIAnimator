import { useEffect, type RefObject } from "react";

// Refraction model: https://kube.io/blog/liquid-glass-css-svg/

const SVG = "http://www.w3.org/2000/svg";

/** What a glass element is given: a filter for its backdrop and its rim highlight. */
interface Lens {
  /** `url(#id)` of the backdrop filter, only while the glass refracts. */
  filter?: string;
  /** The rim as one image stretched over the element, for glass too small to slice. */
  rim?: string;
  /** The rim as a tile whose corners are kept and edges stretched to any size, drawn as a border-image. */
  tile?: { image: string; slice: number; width: number };
}

interface Shape {
  bezel: number;
  thickness: number;
  level: number;
  zoom: number;
  saturation: number;
}

interface Entry {
  lens: Lens;
  node?: Element;
  users: number;
}

// Lenses in the order they were last used. Ones no element uses any more are
// kept up to SPARE, for glass that comes back to a size it had, then evicted
// with their filters.
const lenses = new Map<string, Entry>();
const SPARE = 16;
// Ids are never reused, an evicted filter's id could still be in a stylesheet.
let nextId = 0;
let defs: SVGDefsElement | undefined;

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

function svg(name: string, attributes: Record<string, string | number>): SVGElement {
  const element = document.createElementNS(SVG, name);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, String(value));
  return element;
}

function container(): SVGDefsElement {
  if (defs?.isConnected) return defs;
  const root = svg("svg", { width: 0, height: 0, "aria-hidden": "true" });
  root.style.cssText = "position:absolute;pointer-events:none";
  defs = svg("defs", {}) as SVGDefsElement;
  root.append(defs);
  document.body.append(root);
  return defs;
}

function profile(bezel: number, thickness: number): { samples: Float32Array; maximum: number } {
  const samples = new Float32Array(128);
  let maximum = 0;
  for (let i = 0; i < samples.length; i++) {
    const t = Math.max(0.001, i / (samples.length - 1));
    const surface = (1 - (1 - t) ** 4) ** 0.25;
    const incident = Math.atan((1 - t) ** 3 / surface ** 3);
    const refracted = Math.asin(Math.sin(incident) / 1.5);
    samples[i] = Math.tan(incident - refracted) * (thickness + surface * bezel);
    maximum = Math.max(maximum, samples[i]!);
  }
  return { samples, maximum };
}

const density = () => Math.min(devicePixelRatio || 1, 2);

function canvasOf(image: ImageData): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = image.width;
  canvas.height = image.height;
  canvas.getContext("2d")!.putImageData(image, 0, 0);
  return canvas;
}

// The displacement field (only when `field`, it's only used to refract) and
// the rim highlight, drawn per pixel.
function maps(width: number, height: number, radius: number, selection: boolean, shape: Shape, field: boolean) {
  const scale = density();
  const { bezel, thickness } = shape;
  const { samples, maximum } = profile(bezel, thickness);

  const columns = Math.ceil(width * scale);
  const rows = Math.ceil(height * scale);
  const context = document.createElement("canvas").getContext("2d")!;
  const displacement = field ? context.createImageData(columns, rows) : undefined;
  const shine = context.createImageData(columns, rows);
  const magnify = field && shape.zoom > 1 ? context.createImageData(columns, rows) : undefined;
  const reach = Math.max(width, height) / 2;
  const direction = selection ? 1 : -1;

  for (let y = 0; y < rows; y++) {
    for (let x = 0; x < columns; x++) {
      const px = ((x + 0.5) * width) / columns - width / 2;
      const py = ((y + 0.5) * height) / rows - height / 2;
      const qx = Math.abs(px) - (width / 2 - radius);
      const qy = Math.abs(py) - (height / 2 - radius);
      const ox = Math.max(qx, 0);
      const oy = Math.max(qy, 0);
      const length = Math.hypot(ox, oy);
      const distance = radius - (length + Math.min(Math.max(qx, qy), 0));
      const nx = length > 0 ? (ox / length) * Math.sign(px) : qx > qy ? Math.sign(px) : 0;
      const ny = length > 0 ? (oy / length) * Math.sign(py) : qx > qy ? 0 : Math.sign(py);
      const i = (y * columns + x) * 4;

      if (displacement) {
        let shift = 0;
        if (distance >= 0 && distance < bezel) {
          const sample = (distance / bezel) * (samples.length - 1);
          const lower = Math.floor(sample);
          const a = samples[lower]!;
          const b = samples[Math.min(lower + 1, samples.length - 1)]!;
          shift = a + (b - a) * (sample - lower);
        }
        displacement.data[i] = Math.round(127.5 + ((direction * nx * shift) / maximum) * 127.5);
        displacement.data[i + 1] = Math.round(127.5 + ((direction * ny * shift) / maximum) * 127.5);
        displacement.data[i + 2] = 128;
        displacement.data[i + 3] = 255;
      }

      // A linear field: each point samples from closer to the centre, which
      // enlarges the whole face evenly.
      if (magnify) {
        magnify.data[i] = Math.round(127.5 - (px / reach) * 127.5);
        magnify.data[i + 1] = Math.round(127.5 - (py / reach) * 127.5);
        magnify.data[i + 2] = 128;
        magnify.data[i + 3] = 255;
      }

      const light = -(nx + ny) * Math.SQRT1_2;
      const glint = Math.max(light, 0) ** 3;
      const reflected = Math.max(-light, 0) ** 3;
      shine.data[i] = shine.data[i + 1] = shine.data[i + 2] = 255;

      let alpha = 0;
      if (distance >= 0 && selection) {
        const bands = [0.18, 0.42, 0.66].map((centre) => Math.exp(-(((distance - centre) / 0.22) ** 2)));
        const peak = Math.max(...bands);
        alpha = peak * (0.06 + 0.8 * glint + 0.35 * reflected) * 0.85;
        for (let channel = 0; channel < 3; channel++) {
          shine.data[i + channel] = Math.round(255 * (0.65 + 0.35 * (peak ? bands[channel]! / peak : 0)));
        }
      } else if (distance >= 0) {
        const rim = Math.exp(-(((distance - 0.4) / 0.38) ** 2));
        const bloom = Math.exp(-((distance / (bezel * 0.22)) ** 2));
        alpha = rim * (0.045 + 0.52 * glint + 0.2 * reflected) + bloom * 0.02 * glint;
      }
      shine.data[i + 3] = Math.round(clamp(alpha, 0, 1) * 255);
    }
  }

  return {
    displacement: displacement && canvasOf(displacement),
    shine: canvasOf(shine),
    zoom: magnify && { map: canvasOf(magnify).toDataURL(), scale: 2 * reach * (1 - 1 / shape.zoom) },
    scale: maximum * 2 * shape.level,
  };
}

// The filter only bends the rim. Blur and the rim highlight are applied in CSS,
// which Chromium positions correctly when it repaints part of the glass.
function filterOf(
  width: number,
  height: number,
  selection: boolean,
  shape: Shape,
  scale: number,
  displacement: string,
  highlight: string | undefined,
  zoom: { map: string; scale: number } | undefined,
): { node: Element; filter: string } {
  const id = `glisse-${nextId++}`;
  const box = { x: 0, y: 0, width, height };
  const filter = svg("filter", {
    id,
    ...box,
    filterUnits: "userSpaceOnUse",
    primitiveUnits: "userSpaceOnUse",
    "color-interpolation-filters": "sRGB",
  });

  filter.append(svg("feImage", { href: displacement, ...box, preserveAspectRatio: "none", result: "field" }));

  if (selection) {
    const channels = [
      ["r", 1.035, "1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0"],
      ["g", 1, "0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0"],
      ["b", 0.965, "0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0"],
    ] as const;
    for (const [name, factor, values] of channels) {
      filter.append(
        svg("feDisplacementMap", {
          in: "SourceGraphic",
          in2: "field",
          scale: scale * factor,
          xChannelSelector: "R",
          yChannelSelector: "G",
          result: `${name}-ray`,
        }),
        svg("feColorMatrix", { in: `${name}-ray`, type: "matrix", values, result: name }),
      );
    }
    filter.append(
      svg("feComposite", { in: "r", in2: "g", operator: "arithmetic", k2: 1, k3: 1, result: "rg" }),
      svg("feComposite", { in: "rg", in2: "b", operator: "arithmetic", k2: 1, k3: 1 }),
    );
  } else {
    if (zoom) {
      filter.append(
        svg("feImage", { href: zoom.map, ...box, preserveAspectRatio: "none", result: "zoom" }),
        svg("feDisplacementMap", {
          in: "SourceGraphic",
          in2: "zoom",
          scale: zoom.scale,
          xChannelSelector: "R",
          yChannelSelector: "G",
          result: "magnified",
        }),
      );
    }
    filter.append(
      svg("feDisplacementMap", {
        in: zoom ? "magnified" : "SourceGraphic",
        in2: "field",
        scale,
        xChannelSelector: "R",
        yChannelSelector: "G",
        result: "bent",
      }),
      // The displacement samples the nearest pixel, which leaves stair-steps
      // where the rim bends hardest; a sub-pixel blur smooths them.
      svg("feGaussianBlur", { in: "bent", stdDeviation: 0.4, result: "displaced" }),
      svg("feColorMatrix", { in: "displaced", type: "saturate", values: 1.5, result: "face" }),
    );
    // Along the rim highlight, show the bent backdrop far more saturated, so
    // strong edges break into colour where they cross it.
    if (shape.saturation > 1 && highlight) {
      const mask = svg("feComponentTransfer", { in: "shine", result: "mask" });
      mask.append(svg("feFuncA", { type: "linear", slope: 1.8 }));
      filter.append(
        svg("feColorMatrix", { in: "displaced", type: "saturate", values: shape.saturation, result: "vivid" }),
        svg("feImage", { href: highlight, ...box, preserveAspectRatio: "none", result: "shine" }),
        mask,
        svg("feComposite", { in: "vivid", in2: "mask", operator: "in", result: "edge" }),
        svg("feBlend", { in: "edge", in2: "face", mode: "normal" }),
      );
    }
  }
  container().append(filter);
  return { node: filter, filter: `url("#${id}")` };
}

// One small lens per shape: a square just big enough to hold two corners and
// a strip of edge between them. Past the corners the rim and field only depend
// on the distance to the nearest edge, so any larger glass is the tile's
// corners with its middle row and column stretched along the edges.
interface Tile {
  /** Its corner pieces' side, in css pixels and in the tile's pixels. */
  slice: number;
  pixels: number;
  displacement?: HTMLCanvasElement;
  shine: HTMLCanvasElement;
  rim: string;
  scale: number;
}

const tiles = new Map<string, Tile>();
const TILES = 32;

/** The corner pieces' side for a shape: the bezel or the corner, whichever reaches further in, and some room for the rim's glow. */
const sliceOf = (radius: number, shape: Shape) => Math.ceil(Math.max(radius, shape.bezel)) + 8;

function tileOf(radius: number, shape: Shape, field: boolean): Tile {
  const key = Object.values({ radius, field, scale: density(), ...shape }).join("-");
  const cached = tiles.get(key);
  if (cached) return cached;
  const slice = sliceOf(radius, shape);
  const size = 2 * slice + 1;
  const { displacement, shine, scale } = maps(size, size, radius, false, shape, field);
  const tile = { slice, pixels: Math.round(slice * density()), displacement, shine, rim: shine.toDataURL(), scale };
  tiles.set(key, tile);
  if (tiles.size > TILES) tiles.delete(tiles.keys().next().value!);
  return tile;
}

// A tile stretched to a size: its corners copied as they are, its middle row
// and column drawn along the edges, and `fill` (the field's "no shift")
// in the middle.
function stretch(tile: HTMLCanvasElement, slice: number, width: number, height: number, fill?: string): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(width * density());
  canvas.height = Math.ceil(height * density());
  const context = canvas.getContext("2d")!;
  context.imageSmoothingEnabled = false;
  const { width: w, height: h } = canvas;
  const size = tile.width;
  const middle = size - 2 * slice;
  if (fill) {
    context.fillStyle = fill;
    context.fillRect(0, 0, w, h);
  }
  context.drawImage(tile, 0, 0, slice, slice, 0, 0, slice, slice);
  context.drawImage(tile, size - slice, 0, slice, slice, w - slice, 0, slice, slice);
  context.drawImage(tile, 0, size - slice, slice, slice, 0, h - slice, slice, slice);
  context.drawImage(tile, size - slice, size - slice, slice, slice, w - slice, h - slice, slice, slice);
  context.drawImage(tile, slice, 0, middle, slice, slice, 0, w - 2 * slice, slice);
  context.drawImage(tile, slice, size - slice, middle, slice, slice, h - slice, w - 2 * slice, slice);
  context.drawImage(tile, 0, slice, slice, middle, 0, slice, slice, h - 2 * slice);
  context.drawImage(tile, size - slice, slice, slice, middle, w - slice, slice, slice, h - 2 * slice);
  return canvas;
}

// Builds a lens, from the shape's tile where the glass is big enough to slice.
function build(width: number, height: number, radius: number, selection: boolean, shape: Shape, refracting: boolean, tiled: boolean): Entry {
  if (tiled) {
    const tile = tileOf(radius, shape, refracting);
    const lens: Lens = { tile: { image: `url("${tile.rim}")`, slice: tile.pixels, width: tile.slice } };
    if (!refracting || !tile.displacement) return { lens, users: 0 };
    const displacement = stretch(tile.displacement, tile.pixels, width, height, "rgb(128 128 128)").toDataURL();
    const highlight = shape.saturation > 1 ? stretch(tile.shine, tile.pixels, width, height).toDataURL() : undefined;
    const { node, filter } = filterOf(width, height, selection, shape, tile.scale, displacement, highlight, undefined);
    return { lens: { ...lens, filter }, node, users: 0 };
  }
  const { displacement, shine, zoom, scale } = maps(width, height, radius, selection, shape, refracting);
  const highlight = shine.toDataURL();
  const lens: Lens = { rim: `url("${highlight}")` };
  if (!displacement) return { lens, users: 0 };
  const { node, filter } = filterOf(width, height, selection, shape, scale, displacement.toDataURL(), highlight, zoom);
  return { lens: { ...lens, filter }, node, users: 0 };
}

function acquire(key: string, make: () => Entry): Lens {
  const entry = lenses.get(key) ?? make();
  lenses.delete(key);
  lenses.set(key, entry);
  entry.users++;
  return entry.lens;
}

function release(key: string | undefined): void {
  const entry = key === undefined ? undefined : lenses.get(key);
  if (!entry) return;
  entry.users--;
  let spare = 0;
  for (const other of lenses.values()) if (other.users <= 0) spare++;
  for (const [name, other] of lenses) {
    if (spare <= SPARE) break;
    if (other.users > 0) continue;
    other.node?.remove();
    lenses.delete(name);
    spare--;
  }
}

/** Fits the element's lens to its size and shape, returning the lens' key (or none for a hidden element). */
function fit(element: HTMLElement, selection: boolean, filtered: boolean): string | undefined {
  const width = element.offsetWidth;
  const height = element.offsetHeight;
  if (!width || !height) return undefined;
  const style = getComputedStyle(element);
  const corner = style.borderTopLeftRadius;
  const radius = Math.min(
    width / 2,
    height / 2,
    corner.endsWith("%") ? (Math.min(width, height) * parseFloat(corner)) / 100 : parseFloat(corner),
  );
  const length = (name: string) => {
    const value = parseFloat(style.getPropertyValue(name));
    return Number.isFinite(value) ? value : undefined;
  };
  const shortest = Math.min(width, height);
  const shape = {
    bezel: length("--lg-bezel") ?? clamp(shortest * 0.12, 4, 10),
    thickness: length("--lg-thickness") ?? clamp(shortest * 0.2, 8, 24),
    level: length("--lg-refraction") ?? 1,
    zoom: Math.max(1, length("--lg-zoom") ?? 1),
    saturation: Math.max(1, length("--lg-rim-saturation") ?? 1),
  };
  // Without refraction the tiled lens doesn't depend on the size at all, so
  // every glass of one shape shares it.
  const refracting = filtered && shape.level > 0;
  const tiled = !selection && shape.zoom === 1 && shortest >= 2 * sliceOf(radius, shape) + 1;
  const sized = refracting || !tiled;
  const key = Object.values({ tiled, refracting, radius, selection, density: density(), ...shape, ...(sized && { width, height }) }).join("-");
  const lens = acquire(key, () => build(width, height, radius, selection, shape, refracting, tiled));

  if (lens.filter) element.style.setProperty("--lg-filter", lens.filter);
  else element.style.removeProperty("--lg-filter");
  if (lens.tile) {
    element.style.removeProperty("--lg-rim");
    element.style.setProperty("--lg-rim-tile", lens.tile.image);
    element.style.setProperty("--lg-rim-slice", String(lens.tile.slice));
    element.style.setProperty("--lg-rim-width", `${lens.tile.width}px`);
    element.setAttribute("data-rim", "");
  } else {
    element.style.setProperty("--lg-rim", lens.rim!);
    unsetTile(element);
  }
  return key;
}

function unsetTile(element: HTMLElement): void {
  element.style.removeProperty("--lg-rim-tile");
  element.style.removeProperty("--lg-rim-slice");
  element.style.removeProperty("--lg-rim-width");
  element.removeAttribute("data-rim");
}

// A short wait so a resize that keeps going fits once it stops, then the next
// idle moment where there's one.
function whenIdle(callback: () => void): () => void {
  let idle = 0;
  const timer = setTimeout(() => {
    if ("requestIdleCallback" in window) idle = requestIdleCallback(callback, { timeout: 500 });
    else callback();
  }, 50);
  return () => {
    clearTimeout(timer);
    if (idle) cancelIdleCallback(idle);
  };
}

/** Only Chromium applies SVG filters to the live backdrop. */
export const refractsBackdrop = (): boolean => /Chrom(?:e|ium)\//.test(navigator.userAgent);

// While paused, glass that changes size waits here and refits once it's resumed.
let paused = 0;
const waiting = new Set<() => void>();

/** Holds every lens at the size it had, for a drag that resizes glass continuously. Nests; each call needs a `resumeRefraction`. */
export function pauseRefraction(): void {
  paused++;
}

/** Refits the glass that changed size while paused. */
export function resumeRefraction(): void {
  if (paused === 0 || --paused > 0) return;
  const refits = [...waiting];
  waiting.clear();
  refits.forEach((refit) => refit());
}

// Refits the lens whenever the element changes size. Unfiltered, it sets only
// the rim highlight: the nearest Safari and Firefox come to refraction over
// content they have no copy of.
function refract(element: HTMLElement, selection: boolean, filtered: boolean): () => void {
  let cancel: (() => void) | undefined;
  let current: string | undefined;
  const refit = () => {
    const previous = current;
    current = fit(element, selection, filtered);
    release(previous);
  };
  const schedule = () => {
    cancel?.();
    cancel = undefined;
    if (paused) waiting.add(schedule);
    else cancel = whenIdle(refit);
  };
  const observer = new ResizeObserver(schedule);
  observer.observe(element);
  return () => {
    cancel?.();
    waiting.delete(schedule);
    observer.disconnect();
    release(current);
    element.style.removeProperty("--lg-filter");
    element.style.removeProperty("--lg-rim");
    unsetTile(element);
  };
}

const MARK = "data-glisse-backdrop";

// The part of an element that can be seen, cut down by every ancestor that
// clips its overflow (a large image inside a small scroller shows only the scroller).
function visibleBox(element: HTMLElement): DOMRect {
  let { left, top, right, bottom } = element.getBoundingClientRect();
  for (let node = element.parentElement; node && node !== document.body; node = node.parentElement) {
    const style = getComputedStyle(node);
    if (!/hidden|clip|auto|scroll/.test(`${style.overflow} ${style.overflowX} ${style.overflowY}`)) continue;
    const clip = node.getBoundingClientRect();
    left = Math.max(left, clip.left);
    top = Math.max(top, clip.top);
    right = Math.min(right, clip.right);
    bottom = Math.min(bottom, clip.bottom);
  }
  return new DOMRect(left, top, Math.max(0, right - left), Math.max(0, bottom - top));
}

/** The marked content behind the glass: the closest one whose visible part it overlaps. */
function findBackdrop(element: HTMLElement): HTMLElement | null {
  const box = element.getBoundingClientRect();
  const behind = (candidate: HTMLElement) => {
    if (candidate.contains(element) || element.contains(candidate)) return false;
    const other = visibleBox(candidate);
    return other.left < box.right && other.right > box.left && other.top < box.bottom && other.bottom > box.top;
  };
  for (let node = element.parentElement; node && node !== document.body; node = node.parentElement) {
    const found = [...node.querySelectorAll<HTMLElement>(`[${MARK}]`)].find(behind);
    if (found) return found;
  }
  return null;
}

// A copy never animates on its own (it follows the original instead), and its
// root is placed by the layer rather than where the original sits.
const STILL = /^(transition|animation|view-transition)/;
const PLACED: Record<string, string> = {
  position: "absolute",
  top: "0px",
  left: "0px",
  right: "auto",
  bottom: "auto",
  "margin-top": "0px",
  "margin-right": "0px",
  "margin-bottom": "0px",
  "margin-left": "0px",
  transform: "none",
  translate: "none",
  scale: "none",
  rotate: "none",
};

const kebab = (name: string) => (name.startsWith("--") ? name : name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`));

interface Copy {
  root: HTMLElement;
  /** Each element of the original, with its copy. */
  pairs: Map<Element, HTMLElement>;
}

// Writes an element's computed style onto its copy: every property, or only
// the named ones.
function inline(original: Element, copy: HTMLElement, root: boolean, names?: Iterable<string>): void {
  const computed = getComputedStyle(original);
  const write = (name: string) => {
    if (STILL.test(name) || (root && (name in PLACED || name === "width" || name === "height"))) return;
    const value = computed.getPropertyValue(name);
    if (copy.style.getPropertyValue(name) !== value) copy.style.setProperty(name, value);
  };
  if (names) for (const name of names) write(name);
  else for (let i = 0; i < computed.length; i++) write(computed[i]!);
  if (!root) return;
  for (const [name, value] of Object.entries(PLACED)) copy.style.setProperty(name, value);
  copy.style.width = `${(original as HTMLElement).offsetWidth}px`;
  copy.style.height = `${(original as HTMLElement).offsetHeight}px`;
}

// A deep copy that renders like the original wherever it's placed: each
// element carries its computed style inline, minus anything that would animate
// it or tie it to the original.
function copyOf(source: HTMLElement): Copy {
  const root = source.cloneNode(true) as HTMLElement;
  const originals = [source, ...source.querySelectorAll("*")];
  const copies = [root, ...root.querySelectorAll<HTMLElement>("*")];
  const pairs = new Map<Element, HTMLElement>();
  originals.forEach((original, index) => {
    const copy = copies[index];
    if (!copy) return;
    pairs.set(original, copy);
    copy.removeAttribute("id");
    // Detached, a lazy image would never load.
    if (copy instanceof HTMLImageElement) copy.loading = "eager";
    copy.style.transition = copy.style.animation = "none";
    copy.style.setProperty("view-transition-name", "none");
    inline(original, copy, original === source);
  });
  root.removeAttribute(MARK);
  return { root, pairs };
}

const TIMING = new Set(["offset", "computedOffset", "easing", "composite"]);

// The properties that each running transition or animation in the original
// is changing, by element.
function animating(source: HTMLElement): Map<Element, Set<string>> {
  const found = new Map<Element, Set<string>>();
  if (typeof source.getAnimations !== "function") return found;
  for (const animation of source.getAnimations({ subtree: true })) {
    const effect = animation.effect as KeyframeEffect | null;
    if (!effect?.target || effect.pseudoElement) continue;
    const names = found.get(effect.target) ?? new Set<string>();
    if ("transitionProperty" in animation) names.add(String(animation.transitionProperty));
    else
      for (const keyframe of effect.getKeyframes())
        for (const key of Object.keys(keyframe)) if (!TIMING.has(key)) names.add(kebab(key));
    found.set(effect.target, names);
  }
  return found;
}

const SOURCE = new Set(["src", "srcset", "sizes"]);

// Where the live backdrop can't be filtered, a copy of the marked content is
// filtered instead, in a layer laid just behind the glass and matched to its
// box. The glass then paints its tint, rim and shadows over it, as it would
// over the live backdrop.
function copyBackdrop(element: HTMLElement, source: HTMLElement): () => void {
  const layer = document.createElement("div");
  const frame = document.createElement("div");
  layer.className = "lg-glass-copy";
  layer.setAttribute("aria-hidden", "true");
  layer.setAttribute("inert", "");
  layer.append(frame);
  element.before(layer);
  element.setAttribute("data-copy", "");

  // The layer is a sibling, so it copies the glass's filter, shape and
  // opacity rather than inheriting them. Checked every frame so transitions
  // on the glass carry over.
  const style = getComputedStyle(element);
  let styled = "";
  const restyle = () => {
    const blur = style.getPropertyValue("--lg-blur").trim() || "3px";
    const filter = style.getPropertyValue("--lg-filter").trim() || "saturate(1.5)";
    const next = [`blur(${blur}) ${filter}`, style.borderRadius, style.opacity, style.visibility];
    if (next.join("|") === styled) return;
    styled = next.join("|");
    layer.style.filter = next[0]!;
    layer.style.borderRadius = next[1]!;
    layer.style.clipPath = `inset(0 round ${next[1]})`;
    layer.style.opacity = next[2]!;
    layer.style.visibility = next[3]!;
  };

  let placed = "";
  let shift = { x: 0, y: 0 };
  const align = () => {
    const width = element.offsetWidth;
    const height = element.offsetHeight;
    if (!width || !height || !source.offsetWidth || !source.offsetHeight) {
      layer.style.display = "none";
      return;
    }
    layer.style.display = "";
    const box = element.getBoundingClientRect();
    const from = source.getBoundingClientRect();
    const at = layer.getBoundingClientRect();
    const sx = box.width / width;
    const sy = box.height / height;
    // The layer's untransformed origin, recovered from where it sits now.
    const x = box.left - (at.left - shift.x);
    const y = box.top - (at.top - shift.y);
    const next = [
      width,
      height,
      `translate(${x}px, ${y}px) scale(${sx}, ${sy})`,
      `translate(${(from.left - box.left) / sx}px, ${(from.top - box.top) / sy}px) scale(${
        from.width / source.offsetWidth / sx
      }, ${from.height / source.offsetHeight / sy})`,
    ].join("|");
    if (next === placed) return;
    placed = next;
    shift = { x, y };
    layer.style.width = `${width}px`;
    layer.style.height = `${height}px`;
    layer.style.transform = `translate(${x}px, ${y}px) scale(${sx}, ${sy})`;
    frame.style.transform = next.split("|")[3]!;
  };

  let copy = copyOf(source);
  frame.append(copy.root);
  let stopped = false;

  // Restyles the copy in place, for anything that changes how the original
  // looks without changing what it holds.
  const follow = () => {
    for (const [original, target] of copy.pairs) inline(original, target, original === source);
    placed = "";
    align();
  };

  // Takes a new copy when the original's content changes, keeping the old one
  // up until the new one's images can paint.
  let refresh = 0;
  let version = 0;
  const recopy = () => {
    refresh = 0;
    const next = copyOf(source);
    const id = ++version;
    const images = [...next.root.querySelectorAll("img")];
    if (next.root instanceof HTMLImageElement) images.push(next.root);
    void Promise.all(
      images.map((image) => (typeof image.decode === "function" ? image.decode().catch(() => undefined) : undefined)),
    ).then(() => {
      if (stopped || id !== version) return;
      frame.replaceChildren(next.root);
      copy = next;
      placed = "";
      align();
    });
  };
  const schedule = () => {
    refresh ||= requestAnimationFrame(recopy);
  };

  // Transitions and animations in the original play in the copy too: each
  // frame, whatever they're changing is copied across, and once more after
  // they finish so the copy settles where the original does.
  let moving = new Map<Element, Set<string>>();
  const animate = () => {
    const now = animating(source);
    for (const [original, names] of new Map([...moving, ...now])) {
      const target = copy.pairs.get(original);
      if (target) inline(original, target, original === source, names);
    }
    moving = now;
  };

  // Moving or restyling the glass realigns before the frame is drawn, so the
  // copy never trails it. Scrolls and resizes realign as they happen, and a
  // per-frame check catches anything else that moves either one.
  const moved = new MutationObserver(() => {
    restyle();
    align();
  });
  moved.observe(element, { attributes: true, attributeFilter: ["style", "class", "disabled"] });
  const changed = new MutationObserver((records) => {
    let content = false;
    for (const record of records) {
      const name = record.attributeName;
      if (record.type !== "attributes" || SOURCE.has(name!)) {
        content = true;
        continue;
      }
      const target = copy.pairs.get(record.target as Element);
      if (!target || name === "id" || name === "style" || name === MARK) continue;
      const value = (record.target as Element).getAttribute(name!);
      if (value === null) target.removeAttribute(name!);
      else target.setAttribute(name!, value);
    }
    if (content) schedule();
    follow();
  });
  changed.observe(source, { attributes: true, childList: true, characterData: true, subtree: true });
  // A theme or state set further up, such as data-theme on the page, restyles
  // the original without touching it.
  const context = new MutationObserver(follow);
  for (let node = source.parentElement; node; node = node.parentElement) context.observe(node, { attributes: true });
  const scheme = matchMedia("(prefers-color-scheme: dark)");
  scheme.addEventListener("change", follow);
  const resized = new ResizeObserver(follow);
  resized.observe(source);
  source.addEventListener("load", follow, true);
  source.addEventListener("transitionend", follow);
  document.addEventListener("scroll", align, { capture: true, passive: true });
  window.addEventListener("resize", align);
  restyle();
  align();
  let tick = 0;
  const loop = () => {
    restyle();
    animate();
    align();
    tick = requestAnimationFrame(loop);
  };
  tick = requestAnimationFrame(loop);

  return () => {
    stopped = true;
    cancelAnimationFrame(tick);
    cancelAnimationFrame(refresh);
    moved.disconnect();
    changed.disconnect();
    context.disconnect();
    resized.disconnect();
    scheme.removeEventListener("change", follow);
    source.removeEventListener("load", follow, true);
    source.removeEventListener("transitionend", follow);
    document.removeEventListener("scroll", align, { capture: true });
    window.removeEventListener("resize", align);
    layer.remove();
    element.removeAttribute("data-copy");
  };
}

/**
 * Refracts what's behind the element. Chromium filters the live backdrop.
 * Safari and Firefox filter a copy of content marked `data-glisse-backdrop`
 * nearby, and otherwise keep the blur with the rim highlight.
 */
export function useRefraction(ref: RefObject<HTMLElement | null>, selection = false): void {
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    if (refractsBackdrop()) return refract(element, selection, true);
    // Looking for marked content is a search up every ancestor, so only when there is any.
    const source = selection || !document.querySelector(`[${MARK}]`) ? null : findBackdrop(element);
    if (!source) return refract(element, selection, false);
    const stop = copyBackdrop(element, source);
    const unfit = refract(element, selection, true);
    return () => {
      unfit();
      stop();
    };
  }, [ref, selection]);
}
