import { describe, expect, it } from "vitest";
import { computeZoom, counterZoomBar } from "./zoom";

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
