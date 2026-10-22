import type { ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/utils";

export function Button({ className, type = "button", ...props }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-cyan-400/20 bg-cyan-400/10 px-3 text-sm font-medium text-cyan-200 transition hover:border-cyan-300/40 hover:bg-cyan-400/15 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400/60 disabled:pointer-events-none disabled:opacity-50",
        className,
      )}
      {...props}
    />
  );
}

