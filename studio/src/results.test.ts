import { describe, expect, it } from "vitest";
import { boundedSeriesSamples } from "./results";

describe("bounded result projection", () => {
  it("preserves order, endpoints, and interior extrema within a fixed DOM budget", () => {
    const time = Array.from({ length: 10_000 }, (_, index) => index * 0.01);
    const values = time.map((value) => Math.sin(value));
    values[4_321] = 100;
    values[7_654] = -80;

    const samples = boundedSeriesSamples(time, values, 120);

    expect(samples.length).toBeLessThanOrEqual(120);
    expect(samples[0]?.index).toBe(0);
    expect(samples.at(-1)?.index).toBe(9_999);
    expect(samples.some((sample) => sample.index === 4_321)).toBe(true);
    expect(samples.some((sample) => sample.index === 7_654)).toBe(true);
    expect(samples.map((sample) => sample.index)).toEqual(
      [...samples.map((sample) => sample.index)].sort((left, right) => left - right),
    );
  });

  it("handles tiny budgets without dropping the final sample", () => {
    const time = [0, 1, 2, 3];
    const values = [4, 3, 2, 1];

    expect(boundedSeriesSamples(time, values, 1).map((sample) => sample.index)).toEqual([0]);
    expect(boundedSeriesSamples(time, values, 2).map((sample) => sample.index)).toEqual([0, 3]);
    expect(boundedSeriesSamples(time, values, 3).map((sample) => sample.index)).toEqual([0, 3]);
  });
});
