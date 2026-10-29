import { useEffect, useRef, useState, type CSSProperties, type JSX, type ReactNode } from "react";

import { Button } from "./button";
import { cx } from "./internal/cx";
import { createSpring, type Spring } from "./internal/spring";

export interface MenuItem {
  label: ReactNode;
  icon?: ReactNode;
  onSelect?: () => void;
  destructive?: boolean;
  disabled?: boolean;
}

export type MenuEntry = MenuItem | "separator";

export interface MenuProps {
  items: MenuEntry[];
  /** Names the trigger and the menu for assistive technology. */
  "aria-label": string;
  /** The trigger's icon. Defaults to an ellipsis. */
  icon?: ReactNode;
  /** The trigger edge the menu lines up with as it opens across it. */
  align?: "start" | "end";
  className?: string;
  style?: CSSProperties;
}

type Phase = "closed" | "open" | "leaving";

const clamp = (value: number) => Math.min(1, Math.max(0, value));
const resistance = (value: number) => Math.sign(value) * 7 * (1 - Math.exp((-Math.abs(value) * 0.12) / 7));

const ellipsis = (
  <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden>
    <circle cx="3" cy="8" r="1.4" />
    <circle cx="8" cy="8" r="1.4" />
    <circle cx="13" cy="8" r="1.4" />
  </svg>
);

export function Menu({ items, "aria-label": label, icon = ellipsis, align = "end", className, style }: MenuProps): JSX.Element {
  const [phase, setPhase] = useState<Phase>("closed");
  const triggerRef = useRef<HTMLButtonElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const surfaceRef = useRef<HTMLSpanElement>(null);
  const glyphRef = useRef<HTMLSpanElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const plateRef = useRef<HTMLSpanElement>(null);
  const morphRef = useRef<Spring | null>(null);
  const latest = useRef(items);
  const mounted = phase !== "closed";

  useEffect(() => {
    latest.current = items;
  });

  useEffect(() => {
    if (phase === "open") morphRef.current?.set(1);
    if (phase === "leaving") morphRef.current?.set(0);
  }, [phase]);

  useEffect(() => {
    if (!mounted) return;
    const trigger = triggerRef.current!;
    const card = cardRef.current!;
    const surface = surfaceRef.current!;
    const glyph = glyphRef.current!;
    const content = contentRef.current!;
    const plate = plateRef.current!;

    // As on iOS, the glass grows out of the trigger while the items scale up
    // from its centre and come into focus. Closing runs it back.
    const from = { width: trigger.offsetWidth, height: trigger.offsetHeight };
    const to = { width: card.offsetWidth, height: card.offsetHeight };
    const radius = parseFloat(getComputedStyle(surface).borderTopLeftRadius);
    const trailing = card.dataset.align !== "start";
    const origin = { x: trailing ? to.width - from.width / 2 : from.width / 2, y: from.height / 2 };
    content.style.transformOrigin = `${origin.x}px ${origin.y}px`;
    glyph.style.width = `${from.width}px`;
    glyph.style.height = `${from.height}px`;
    let leaving = false;
    const shape = createSpring(0, 600, 46, (value) => {
      const t = clamp(value);
      const settled = t === 1;
      const width = from.width + (to.width - from.width) * t;
      const height = from.height + (to.height - from.height) * t;
      const scale = 0.5 + 0.5 * t;
      // The items stay inside the glass: clip them to its rect, mapped back
      // through their own scale.
      const local = (edge: number, centre: number) => centre + (edge - centre) / scale;
      const left = local(trailing ? to.width - width : 0, origin.x);
      const right = local(trailing ? to.width : width, origin.x);
      const bottom = local(height, origin.y);
      const top = local(0, origin.y);
      surface.style.width = settled ? "" : `${width}px`;
      surface.style.height = settled ? "" : `${height}px`;
      surface.style.opacity = String(clamp(t / 0.2));
      trigger.style.opacity = t === 0 ? "" : String(1 - clamp(t / 0.2));
      // The trigger's icon rides the glass towards its centre, swelling and
      // blurring away as the items arrive.
      const drift = 0.3 * t;
      const dx = ((trailing ? to.width - width / 2 : width / 2) - origin.x) * drift;
      const dy = (height / 2 - origin.y) * drift;
      glyph.style.opacity = String(1 - clamp(t / 0.45));
      glyph.style.transform = `translate(${dx}px, ${dy}px) scale(${1 + 0.6 * t})`;
      glyph.style.filter = `blur(${8 * t}px)`;
      content.style.opacity = String(leaving ? clamp((t - 0.55) / 0.45) : clamp((t - 0.15) / 0.6));
      content.style.transform = settled ? "" : `scale(${scale})`;
      content.style.filter = settled ? "" : `blur(${6 * (1 - t)}px)`;
      content.style.clipPath = settled
        ? ""
        : `inset(${top}px ${to.width - right}px ${to.height - bottom}px ${left}px round ${radius / scale}px)`;
      if (leaving && value === 0) setPhase("closed");
    });
    const morph: Spring = {
      ...shape,
      set(target, instant) {
        leaving = target === 0;
        if (leaving) shape.tune(900, 57);
        else shape.tune(600, 46);
        shape.set(target, instant);
      },
    };
    morphRef.current = morph;
    shape.paint();
    morph.set(1);
    const rows = () => [...card.querySelectorAll<HTMLButtonElement>(".lg-menu-item:not(:disabled)")];
    rows()[0]?.focus({ preventScroll: true });

    const offset = { scale: 1, x: 0, y: 0 };
    const paint = () => {
      card.style.scale = String(offset.scale);
      card.style.translate = `${offset.x}px ${offset.y}px`;
    };
    const scale = createSpring(1, 650, 36, (value) => {
      offset.scale = value;
      paint();
    });
    const x = createSpring(0, 600, 30, (value) => {
      offset.x = value;
      paint();
    });
    const y = createSpring(0, 600, 30, (value) => {
      offset.y = value;
      paint();
    });

    let gesture: { id: number; x: number; y: number } | null = null;
    let current: HTMLButtonElement | null = null;
    let suppressClickUntil = 0;

    const close = (restoreFocus = true) => {
      setPhase("leaving");
      if (restoreFocus) triggerRef.current?.focus({ preventScroll: true });
    };

    // The plate slides between rows for a drag and for a hovering mouse alike.
    const highlight = (row: HTMLButtonElement | null) => {
      if (current === row) return;
      const first = !current;
      current = row;
      if (!row) {
        delete card.dataset.scrubbing;
        plate.style.opacity = "0";
        return;
      }
      card.dataset.scrubbing = "";
      plate.style.width = `${row.offsetWidth}px`;
      plate.style.height = `${row.offsetHeight}px`;
      // The first row lights up in place and at once, taking over from the
      // row's own hover background without a gap; later rows slide to it.
      if (first) plate.style.transition = "none";
      plate.style.transform = `translate(${row.offsetLeft}px, ${row.offsetTop}px)`;
      plate.style.opacity = "1";
      if (first) {
        void plate.offsetWidth;
        plate.style.transition = "";
      }
    };

    const rowAt = (clientX: number, clientY: number) => {
      const row = document.elementFromPoint(clientX, clientY)?.closest<HTMLButtonElement>(".lg-menu-item");
      return row && !row.disabled && card.contains(row) ? row : null;
    };

    const down = (event: PointerEvent) => {
      if (!event.isPrimary || event.button !== 0) return;
      gesture = { id: event.pointerId, x: event.clientX, y: event.clientY };
      card.setPointerCapture(event.pointerId);
      card.dataset.pressed = "";
      scale.set(1 + 5 / Math.max(card.offsetWidth, card.offsetHeight, 200));
      highlight(rowAt(event.clientX, event.clientY));
    };

    const move = (event: PointerEvent) => {
      if (!gesture) {
        // Crossing a separator keeps the plate on the last row, so it slides
        // across the gap to the next one instead of fading out and reappearing.
        const row = event.pointerType === "mouse" ? rowAt(event.clientX, event.clientY) : null;
        if (row) highlight(row);
        return;
      }
      if (event.pointerId !== gesture.id) return;
      highlight(rowAt(event.clientX, event.clientY));
      x.set(resistance(event.clientX - gesture.x), true);
      y.set(resistance(event.clientY - gesture.y), true);
    };

    const end = (event: PointerEvent) => {
      if (event.pointerId !== gesture?.id) return;
      const chosen = event.type === "pointerup" ? rowAt(event.clientX, event.clientY) : null;
      highlight(null);
      gesture = null;
      delete card.dataset.pressed;
      scale.set(1);
      x.set(0);
      y.set(0);
      suppressClickUntil = performance.now() + 500;
      chosen?.click();
    };

    const leave = (event: PointerEvent) => {
      if (!gesture && event.pointerType === "mouse") highlight(null);
    };

    const click = (event: MouseEvent) => {
      const row = (event.target as Element).closest<HTMLButtonElement>(".lg-menu-item");
      if (!row || (event.detail !== 0 && performance.now() < suppressClickUntil)) return;
      const item = latest.current[Number(row.dataset.index)];
      if (!item || item === "separator") return;
      close();
      item.onSelect?.();
    };

    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        close();
      } else if (event.key === "Tab") {
        close(false);
      } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
        event.preventDefault();
        if (!gesture) highlight(null);
        const list = rows();
        const index = list.indexOf(document.activeElement as HTMLButtonElement);
        const next =
          event.key === "Home"
            ? 0
            : event.key === "End"
              ? list.length - 1
              : (index + (event.key === "ArrowDown" ? 1 : -1) + list.length) % list.length;
        list[next]?.focus();
      }
    };

    const outside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!card.contains(target) && !triggerRef.current?.contains(target)) close(false);
    };

    card.addEventListener("pointerdown", down);
    card.addEventListener("pointermove", move);
    card.addEventListener("pointerleave", leave);
    card.addEventListener("pointerup", end);
    card.addEventListener("pointercancel", end);
    card.addEventListener("lostpointercapture", end);
    card.addEventListener("click", click);
    card.addEventListener("keydown", key);
    document.addEventListener("pointerdown", outside, true);
    return () => {
      [shape, scale, x, y].forEach((spring) => spring.stop());
      morphRef.current = null;
      trigger.style.opacity = "";
      card.removeEventListener("pointerdown", down);
      card.removeEventListener("pointermove", move);
      card.removeEventListener("pointerleave", leave);
      card.removeEventListener("pointerup", end);
      card.removeEventListener("pointercancel", end);
      card.removeEventListener("lostpointercapture", end);
      card.removeEventListener("click", click);
      card.removeEventListener("keydown", key);
      document.removeEventListener("pointerdown", outside, true);
    };
  }, [mounted]);

  const open = phase === "open";

  return (
    <div className={cx("lg-menu", className)} style={style}>
      <Button
        ref={triggerRef}
        icon={icon}
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setPhase(open ? "leaving" : "open")}
      />
      {mounted ? (
        <div ref={cardRef} role="menu" aria-label={label} className="lg-menu-card" data-phase={phase} data-align={align}>
          <span ref={surfaceRef} className="lg-menu-surface" aria-hidden />
          <span ref={glyphRef} className="lg-menu-glyph" aria-hidden>
            {icon}
          </span>
          <div ref={contentRef} role="none" className="lg-menu-content">
            <span ref={plateRef} className="lg-menu-plate" aria-hidden />
            {items.map((item, index) =>
              item === "separator" ? (
                <div key={index} role="separator" className="lg-menu-separator" />
              ) : (
                <button
                  key={index}
                  type="button"
                  role="menuitem"
                  className="lg-menu-item"
                  data-index={index}
                  data-destructive={item.destructive ? "" : undefined}
                  disabled={item.disabled}
                >
                  {item.icon}
                  {item.label}
                </button>
              ),
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
