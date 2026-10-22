import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatBitsPerSecond(value: number): string {
  if (!Number.isFinite(value) || value <= 0 || value > 1_000_000_000_000_000) return "—";
  const units = ["bps", "Kbps", "Mbps", "Gbps", "Tbps"];
  const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1000)), units.length - 1);
  const amount = value / 1000 ** exponent;
  return `${amount >= 100 || exponent === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[exponent]}`;
}

export function formatBytes(value: number): string {
  if (value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** exponent).toFixed(exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

export function formatThroughput(bytesPerSecond: number): string {
  const bitsPerSecond = bytesPerSecond * 8;
  if (bitsPerSecond < 1_000) return `${bitsPerSecond.toFixed(0)} bps`;
  if (bitsPerSecond < 1_000_000) return `${(bitsPerSecond / 1_000).toFixed(1)} Kbps`;
  if (bitsPerSecond < 1_000_000_000) return `${(bitsPerSecond / 1_000_000).toFixed(1)} Mbps`;
  return `${(bitsPerSecond / 1_000_000_000).toFixed(2)} Gbps`;
}

