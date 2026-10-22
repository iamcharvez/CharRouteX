import type { HTMLAttributes } from "react";
import { cn } from "../../lib/utils";

export function Card({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("panel rounded-2xl border border-white/[0.07]", className)} {...props} />;
}

