import {
  forwardRef,
  useImperativeHandle,
  useRef,
  type ForwardRefExoticComponent,
  type HTMLAttributes,
  type RefAttributes,
} from "react";

import { cx } from "./internal/cx";
import { useRefraction } from "./internal/refraction";

export type GlassProps = HTMLAttributes<HTMLDivElement>;

export const Glass: ForwardRefExoticComponent<GlassProps & RefAttributes<HTMLDivElement>> = /* @__PURE__ */ forwardRef<
  HTMLDivElement,
  GlassProps
>(function Glass({ className, ...props }, forwardedRef) {
  const ref = useRef<HTMLDivElement>(null);
  useImperativeHandle(forwardedRef, () => ref.current!);
  useRefraction(ref);
  return <div ref={ref} className={cx("lg-glass", className)} {...props} />;
});
