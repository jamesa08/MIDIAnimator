import { useCallback, useEffect, useRef, useState } from "react";

export function useControllable<T, U extends T = T>(
  value: T | undefined,
  defaultValue: T,
  onChange: ((value: U) => void) | undefined,
): [T, (next: U) => void] {
  const [uncontrolled, setUncontrolled] = useState(defaultValue);
  const controlled = value !== undefined;
  const handler = useRef(onChange);

  useEffect(() => {
    handler.current = onChange;
  });

  const set = useCallback(
    (next: U) => {
      if (!controlled) setUncontrolled(next);
      handler.current?.(next);
    },
    [controlled],
  );

  return [controlled ? value : uncontrolled, set];
}
