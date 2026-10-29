import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  type CSSProperties,
  type ForwardRefExoticComponent,
  type InputHTMLAttributes,
  type RefAttributes,
} from "react";

import { useControllable } from "./internal/controllable";
import { cx } from "./internal/cx";
import { createSpring } from "./internal/spring";

export interface SliderProps
  extends Omit<
    InputHTMLAttributes<HTMLInputElement>,
    "type" | "value" | "defaultValue" | "onChange" | "min" | "max" | "step"
  > {
  value?: number;
  defaultValue?: number;
  onValueChange?: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
}

export const Slider: ForwardRefExoticComponent<SliderProps & RefAttributes<HTMLInputElement>> = /* @__PURE__ */ forwardRef<
  HTMLInputElement,
  SliderProps
>(function Slider(
  { value: valueProp, defaultValue, onValueChange, min = 0, max = 100, step = 1, className, style, ...props },
  forwardedRef,
) {
  const inputRef = useRef<HTMLInputElement>(null);
  const thumbRef = useRef<HTMLSpanElement>(null);
  const faceRef = useRef<HTMLSpanElement>(null);
  const [value, setValue] = useControllable(valueProp, defaultValue ?? min, onValueChange);
  const progress = max > min ? Math.min(1, Math.max(0, (value - min) / (max - min))) : 0;
  useImperativeHandle(forwardedRef, () => inputRef.current!);

  useEffect(() => {
    const input = inputRef.current!;
    const thumb = thumbRef.current!;
    const face = faceRef.current!;
    const lift = createSpring(0, 2000, 80, (amount) => {
      thumb.style.scale = String(0.6 + 0.4 * amount);
      face.style.opacity = String(1 - 0.85 * Math.min(1, Math.max(0, amount)));
    });

    const hold = () => {
      if (!input.disabled) lift.set(1);
    };
    const release = () => lift.set(0);
    const ends = ["pointerup", "pointercancel", "lostpointercapture", "blur"] as const;

    input.addEventListener("pointerdown", hold);
    ends.forEach((type) => input.addEventListener(type, release));
    window.addEventListener("pointerup", release);
    return () => {
      lift.stop();
      input.removeEventListener("pointerdown", hold);
      ends.forEach((type) => input.removeEventListener(type, release));
      window.removeEventListener("pointerup", release);
    };
  }, []);

  return (
    <div className={cx("lg-slider", className)} style={{ ...style, "--lg-progress": progress } as CSSProperties}>
      <span className="lg-slider-track" aria-hidden>
        <span className="lg-slider-fill" />
      </span>
      <span ref={thumbRef} className="lg-slider-thumb" aria-hidden>
        <span ref={faceRef} className="lg-thumb-face" />
      </span>
      <input
        ref={inputRef}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => setValue(event.currentTarget.valueAsNumber)}
        {...props}
      />
    </div>
  );
});
