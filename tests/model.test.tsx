import { expect, test } from "nsh/test";
import { segments, valueDomain } from "../src/chart";
import {
  CAPACITY,
  avatarToken,
  bytes,
  containerTone,
  duration,
  emptyHistory,
  groupContainers,
  initials,
  orderApps,
  rate,
  reading,
  record,
  splitUnit,
} from "../src/model";
import { app, extras, reading as sample } from "./fixtures";

test("unavailable readings never look like a measured zero", () => {
  expect(reading(null, bytes)).toBe("Unavailable");
  expect(reading(0, rate)).toBe("0 B/s");
  expect(bytes(512e6)).toBe("512 MB");
  expect(bytes(3.254e9)).toBe("3.25 GB");
  expect(rate(1500)).toBe("1.5 kB/s");
  expect(rate(2e6)).toBe("2.0 MB/s");
  expect(duration(90000)).toBe("1d 1h");
  expect(duration(3900)).toBe("1h 5m");
  expect(splitUnit("18.40 GB")).toEqual(["18.40", "GB"]);
  expect(splitUnit("2.4 GB/s")).toEqual(["2.4", "GB/s"]);
  expect(splitUnit("Unavailable")).toEqual(["Unavailable", ""]);
});

test("history keeps the latest samples and leaves gaps for missing values", () => {
  let h = emptyHistory;
  for (let i = 0; i < CAPACITY + 5; i++) h = record(h, sample(i * 2));
  expect(h.cpu.length).toBe(CAPACITY);
  expect(h.cpu[0]!.t).toBe(10);
  expect(h.memory[0]!.v).toBe(37.5);
  h = record(h, sample(200, { cpu: null, network: null }));
  expect(h.cpu[h.cpu.length - 1]!.t).toBe((CAPACITY + 4) * 2);
  expect(h.download[h.download.length - 1]!.t).toBe((CAPACITY + 4) * 2);
  expect(h.memory[h.memory.length - 1]!.t).toBe(200);
});

test("a pause splits the chart into separate runs", () => {
  // Ages, oldest first: samples at 2 s, 4 s and 30 s.
  expect(segments([28, 26, 0])).toEqual([
    [0, 2],
    [2, 3],
  ]);
  expect(segments([4, 2, 0])).toEqual([[0, 3]]);
  expect(segments([])).toEqual([]);
});

test("value axes fit their units", () => {
  expect(valueDomain([10, 20], [], "percent", false)).toEqual([0, 100]);
  expect(valueDomain([10, 20], [], "storage", false)).toEqual([0, 100]);
  const [lo, hi] = valueDomain([40, 42], [], "percent", true);
  expect(lo < 40 && hi > 42 && lo >= 0 && hi <= 100).toBe(true);
  expect(valueDomain([], [], "rate", false)[0]).toBe(0);
  // Upload shares the scale, so a larger upload raises it.
  expect(valueDomain([10_000], [50_000], "rate", false)[1] > 50_000).toBe(true);
});

test("search is case-insensitive and sorting is stable", () => {
  const apps = [app("Safari", 8, 400), app("Chrome", 20, 300), app("Terminal", 0, 100)];
  expect(orderApps(apps, "  SAF ", "cpu", true).map((a) => a.name)).toEqual(["Safari"]);
  expect(orderApps(apps, "", "memory", true).map((a) => a.name)).toEqual(["Safari", "Chrome", "Terminal"]);
  const ties = [app("Zulu", 1, 10), app("Alpha", 1, 10)];
  expect(orderApps(ties, "", "cpu", true)[0]!.name).toBe("Alpha");
  expect(orderApps(ties, "", "name", true)[0]!.name).toBe("Zulu");
  const twins = [{ ...app("Editor", 1, 1), id: "b" }, { ...app("Editor", 1, 1), id: "a" }];
  expect(orderApps(twins, "", "name", false).map((a) => a.id)).toEqual(["a", "b"]);
});

test("avatars and container states", () => {
  expect(initials("Visual Studio Code")).toBe("VS");
  expect(initials("")).toBe("A");
  expect(avatarToken("Firefox")).toBe(avatarToken("Firefox"));
  expect(containerTone("Up 2 hours (healthy)")).toBe("healthy");
  expect(containerTone("Up 2 hours (unhealthy)")).toBe("danger");
  expect(containerTone("Restarting (1) 30 seconds ago")).toBe("warning");
  expect(containerTone("Up 2 hours (Paused)")).toBe("warning");
  expect(containerTone("Exited (1)")).toBe("muted");
  const groups = groupContainers([...extras.containers, { ...extras.containers[0]!, name: "cache", project: "" }]);
  expect(groups.map((g) => g.name)).toEqual(["shop", "Standalone"]);
});
