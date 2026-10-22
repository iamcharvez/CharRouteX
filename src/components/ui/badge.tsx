import type { HTMLAttributes } from "react";
import { cn } from "../../lib/utils";

export function Badge({ className, ...props }: HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      className={cn("inline-flex items-center gap-1.5 rounded-full border px-2 py-1 text-[11px] font-semibold uppercase tracking-[0.12em]", className)}
      {...props}
    />
  );
}

