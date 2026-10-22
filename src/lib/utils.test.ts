import { describe, expect, it } from "vitest";
import { formatBitsPerSecond, formatBytes, formatThroughput } from "./utils";

describe("network display formatting", () => {
  it("formats link speeds using decimal networking units", () => {
    expect(formatBitsPerSecond(1_000_000_000)).toBe("1.0 Gbps");
    expect(formatBitsPerSecond(0)).toBe("—");
    expect(formatBitsPerSecond(Number.MAX_VALUE)).toBe("—");
  });

  it("formats cumulative byte counters using binary units", () => {
    expect(formatBytes(1_048_576)).toBe("1.0 MB");
    expect(formatBytes(0)).toBe("0 B");
  });

  it("formats sampled byte counters as network-standard bits per second", () => {
    expect(formatThroughput(40_600_000)).toBe("324.8 Mbps");
  });
});

