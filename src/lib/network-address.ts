export function prefixToMask(prefix: number): string {
  const normalized = Math.max(0, Math.min(32, Math.trunc(prefix)));
  const bits = normalized === 0 ? 0 : (0xffffffff << (32 - normalized)) >>> 0;
  return [24, 16, 8, 0].map((shift) => (bits >>> shift) & 255).join(".");
}

export function maskToPrefix(value: string): number | null {
  const parts = value.trim().split(".").map(Number);
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) return null;
  const bits = parts.map((part) => part.toString(2).padStart(8, "0")).join("");
  if (!/^1*0*$/.test(bits)) return null;
  const firstZero = bits.indexOf("0");
  return firstZero === -1 ? 32 : firstZero;
}
