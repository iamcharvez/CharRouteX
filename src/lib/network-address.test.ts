import { describe, expect, it } from "vitest";
import { maskToPrefix, prefixToMask } from "./network-address";

describe("IPv4 subnet conversion", () => {
  it.each([
    [0, "0.0.0.0"],
    [8, "255.0.0.0"],
    [24, "255.255.255.0"],
    [31, "255.255.255.254"],
    [32, "255.255.255.255"],
  ])("converts /%i to %s", (prefix, mask) => {
    expect(prefixToMask(prefix)).toBe(mask);
    expect(maskToPrefix(mask)).toBe(prefix);
  });

  it("rejects malformed and non-contiguous subnet masks", () => {
    expect(maskToPrefix("255.0.255.0")).toBeNull();
    expect(maskToPrefix("255.255.255.256")).toBeNull();
    expect(maskToPrefix("255.255.255")).toBeNull();
  });
});
