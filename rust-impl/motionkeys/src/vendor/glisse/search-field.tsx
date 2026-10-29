import { forwardRef, type ForwardRefExoticComponent, type RefAttributes } from "react";

import { Input, type InputProps } from "./input";

export type SearchFieldProps = Omit<InputProps, "type">;

const magnifier = (
  <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" aria-hidden>
    <circle cx="7" cy="7" r="4.75" />
    <path d="m10.5 10.5 3.25 3.25" />
  </svg>
);

/** An `Input` for searching: a magnifier icon, a "Search" placeholder and a clear button. */
export const SearchField: ForwardRefExoticComponent<SearchFieldProps & RefAttributes<HTMLInputElement>> = /* @__PURE__ */ forwardRef<
  HTMLInputElement,
  SearchFieldProps
>(function SearchField({ icon = magnifier, placeholder = "Search", ...props }, ref) {
  return <Input ref={ref} type="search" icon={icon} placeholder={placeholder} {...props} />;
});
