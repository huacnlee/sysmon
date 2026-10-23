import { expect, open, test } from "nsh/test";

// Check observable navigation and controls, without asserting machine-specific
// process names, hardware capacities, or fluctuating utilization values.
test("opens overview and navigates live system detail pages", async () => {
  const app = await open(".", { width: 1280, height: 800, platform: "macos" });
  expect(app.texts()).toContain("sysmon");
  expect(app.shows("id:sysmon-titlebar")).toBe(true);
  await app.waitFor("Memory by type");
  expect(app.texts()).toContain("⊞  Memory by app");

  await app.press("id:nav-CPU");
  await app.waitFor("Processor utilization");
  expect(app.texts()).toContain("⊞  CPU by app");

  await app.press("id:nav-Memory");
  await app.waitFor("Physical memory in use");
  expect(app.texts()).toContain("⊞  Memory by app");

  await app.press("id:nav-Disk");
  await app.waitFor("Available storage");
  await app.press("id:nav-Network");
  await app.waitFor("Download");

  await app.press("id:nav-Overview");
  await app.waitFor("Memory by type");
});

test("pauses sampling until resumed", async () => {
  const app = await open(".", { width: 1280, height: 800, platform: "macos" });
  await app.waitFor("Memory by type");
  expect(app.shows("id:sysmon-titlebar")).toBe(true);
  await app.press("id:sampling-toggle");
  await app.waitFor("Ⅱ Paused");
  expect(app.texts()).toContain("Resume");
  const frozen = app.texts();
  await app.wait(2400);
  expect(app.texts()).toEqual(frozen);
  await app.press("id:sampling-toggle");
  await app.waitFor("● Live · 2s");
  expect(app.texts()).toContain("Pause");
});

test("opens projects, assistants, and container pages", async () => {
  const app = await open(".", { width: 1280, height: 800, platform: "macos" });
  await app.press("id:nav-Projects");
  await app.waitFor("Development projects");
  await app.press("id:nav-Agents");
  await app.waitFor("Your coding assistants");
  await app.press("id:nav-Containers");
  await app.waitFor("⬡  Containers");
  await app.press("id:nav-Overview");
  await app.waitFor("Memory by type");
});


test("keeps navigation and sampling controls usable on phone layouts", async () => {
  const app = await open(".", {
    width: 390, height: 844, platform: "ios",
    safeArea: { top: 47, bottom: 34 },
  });
  expect(app.shows("id:sysmon-titlebar")).toBe(false);
  await app.waitFor("Memory by type");
  await app.press("Pause");
  expect(app.texts()).toContain("Resume");
  await app.press("id:nav-CPU");
  await app.waitFor("Processor utilization");
  await app.setViewport(844, 390);
  expect(app.shows("id:sysmon-titlebar")).toBe(false);
  expect(app.texts()).toContain("Processor utilization");
  await app.press("Resume");
  expect(app.texts()).toContain("Pause");
});

test("preserves application search and sort across live updates and opens resource details", async () => {
  const app = await open(".", { width: 1280, height: 800, platform: "macos" });
  await app.press("id:nav-CPU");
  await app.waitFor("id:apps-search");

  await app.press("id:sort-memory");
  await app.waitFor("Memory ↓");
  const missing = "__sysmon_missing_application__";
  await app.type("id:apps-search", missing);
  await app.waitFor("No matching applications");
  expect(app.count("label:/^Open details for /")).toBe(0);
  await app.wait(6100);
  expect(app.value("id:apps-search")).toBe(missing);
  expect(app.texts()).toContain("No matching applications");
  expect(app.texts()).toContain("Memory ↓");

  await app.press("id:apps-clear-search");
  await app.waitUntil(() => app.value("id:apps-search") === "");
  await app.waitFor("label:/^Open details for / >> nth=0");
  await app.press("id:sort-name");
  await app.waitFor("Name ↑");
  await app.press("id:sort-name");
  await app.waitFor("Name ↓");

  // Freeze the list before selecting a machine-dependent process name.
  await app.press("Pause");
  await app.press("label:/^Open details for / >> nth=0");
  await app.waitFor("Resident memory");
  expect(app.texts()).toContain("Application total across the grouped processes. CPU 100% equals one full core; shared memory pages may overlap.");
  await app.press("label:/^Close details for / >> nth=0");
  await app.waitFor("Application total across the grouped processes. CPU 100% equals one full core; shared memory pages may overlap.", { state: "gone" });
});
