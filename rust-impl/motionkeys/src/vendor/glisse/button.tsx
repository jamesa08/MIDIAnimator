import {
  forwardRef,
  useImperativeHandle,
  useRef,
  type ButtonHTMLAttributes,
  type CSSProperties,
  type ForwardRefExoticComponent,
  type ReactNode,
  type RefAttributes,
} from "react";

import { cx } from "./internal/cx";
import { usePress } from "./internal/press";
import { useRefraction } from "./internal/refraction";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Shown before the label. On its own, the button is a circle; give it an `aria-label`. */
  icon?: ReactNode;
  /** `glass` is clear; `filled` fills it with the accent colour, for the primary action. */
  variant?: "glass" | "filled";
  /** Marks an action that deletes or can't be undone. Shorthand for a red `tint`. */
  destructive?: boolean;
  /** Any CSS colour. Tints a glass button's label, or a filled button's fill. */
  tint?: string;
}

export const Button: ForwardRefExoticComponent<ButtonProps & RefAttributes<HTMLButtonElement>> = /* @__PURE__ */ forwardRef<
  HTMLButtonElement,
  ButtonProps
>(function Button(
  { icon, variant = "glass", destructive, tint, children, className, style, type = "button", ...props },
  forwardedRef,
) {
  const ref = useRef<HTMLButtonElement>(null);
  useImperativeHandle(forwardedRef, () => ref.current!);
  usePress(ref);
  useRefraction(ref);

  return (
    <button
      ref={ref}
      type={type}
      className={cx("lg-glass lg-button", className)}
      data-icon={icon == null ? undefined : children == null ? "only" : ""}
      data-variant={variant === "glass" ? undefined : variant}
      data-destructive={destructive ? "" : undefined}
      data-tint={tint ? "" : undefined}
      style={tint ? ({ ...style, "--lg-tint": tint } as CSSProperties) : style}
      {...props}
    >
      {icon}
      {children}
    </button>
  );
});
