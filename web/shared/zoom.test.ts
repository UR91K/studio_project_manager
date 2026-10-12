import { describe, expect, it } from "vitest";
import { computeZoom, counterZoomBar, needsRestore, readStoredZoom, STORAGE_KEY, writeStoredZoom } from "./zoom";

describe("computeZoom", () => {
  it("is the pixel ratio over the display scale", () => {
    expect(computeZoom(1, 1)).toBe(1);
    expect(computeZoom(2, 1)).toBe(2);
    expect(computeZoom(1.25, 1.25)).toBe(1);
    expect(computeZoom(2.5, 1.25)).toBe(2);
    expect(computeZoom(0.25, 1)).toBe(0.25);
  });
  it("falls back to 1 on unusable input", () => {
    expect(computeZoom(NaN, 1)).toBe(1);
    expect(computeZoom(1, 0)).toBe(1);
    expect(computeZoom(0, 1)).toBe(1);
  });
});

describe("counterZoomBar", () => {
  it("counter-zooms the bar only below 100 percent", () => {
    expect(counterZoomBar(0.25)).toBe(true);
    expect(counterZoomBar(0.99)).toBe(true);
    expect(counterZoomBar(1)).toBe(false);
    expect(counterZoomBar(5)).toBe(false);
  });
});

describe("stored zoom", () => {
  const fake = (value: string | null) => ({
    getItem: (k: string) => (k === STORAGE_KEY ? value : null),
    setItem: () => {},
  });
  const recording = () => {
    const data: Record<string, string> = {};
    return { data, getItem: (k: string) => data[k] ?? null, setItem: (k: string, v: string) => void (data[k] = v) };
  };
  const throwing = {
    getItem: () => { throw new Error("blocked"); },
    setItem: () => { throw new Error("blocked"); },
  };

  it("reads a valid value", () => {
    expect(readStoredZoom(fake("2"))).toBe(2);
    expect(readStoredZoom(fake("0.8"))).toBe(0.8);
    expect(readStoredZoom(fake("0.25"))).toBe(0.25);
    expect(readStoredZoom(fake("5"))).toBe(5);
  });
  it("ignores missing, garbage and out-of-range values", () => {
    for (const v of [null, "", "  ", "abc", "NaN", "Infinity", "0", "-1", "0.24", "5.01", "100"]) {
      expect(readStoredZoom(fake(v))).toBeNull();
    }
  });
  it("tolerates storage that throws or is absent", () => {
    expect(readStoredZoom(throwing)).toBeNull();
    expect(readStoredZoom(null)).toBeNull();
    expect(() => writeStoredZoom(2, throwing)).not.toThrow();
    expect(() => writeStoredZoom(2, null)).not.toThrow();
  });
  it("writes rounded values under the key and skips unusable ones", () => {
    const s = recording();
    writeStoredZoom(1.2000000000000002, s);
    expect(s.data[STORAGE_KEY]).toBe("1.2");
    writeStoredZoom(NaN, s);
    writeStoredZoom(9, s);
    expect(s.data[STORAGE_KEY]).toBe("1.2");
    expect(readStoredZoom(s)).toBe(1.2);
  });
  it("restores only a saved factor that is not 100 percent", () => {
    expect(needsRestore(null)).toBe(false);
    expect(needsRestore(1)).toBe(false);
    expect(needsRestore(1.00001)).toBe(false);
    expect(needsRestore(1.2)).toBe(true);
    expect(needsRestore(0.5)).toBe(true);
  });
});
