import { expect, open, test } from "nsh/test";
import { nativeModule } from "./fixtures";

const desktop = { width: 1120, height: 760, platform: "macos" as const };

test("the overview shows live readings and opens a resource", async () => {
  const app = await open("../App.tsx", { ...desktop, modules: nativeModule() });
  await app.waitFor("id:page-overview");
  await app.waitFor("text:Device snapshot");
  await app.waitFor("text:Firefox");
  expect(app.shows("text:workstation")).toBe(true);
  expect(app.shows("text:82% · Discharging")).toBe(true);
  await app.press("id:card-cpu");
  await app.waitFor("id:page-cpu");
  await app.waitFor("id:app-table");
  await app.press("label:Overview");
  await app.waitFor("id:page-overview");
});

test("the application table searches, sorts and expands rows", async () => {
  const app = await open("../App.tsx", { ...desktop, modules: nativeModule() });
  await app.waitFor("id:page-overview");
  await app.press("id:titlebar-memory");
  await app.waitFor("text:4 applications");
  await app.type("id:apps-search", "fire");
  await app.waitFor("text:1 of 4 applications");
  expect(app.shows("text:Slack")).toBe(false);
  await app.press("id:apps-clear-search");
  await app.waitFor("text:4 applications");
  await app.press("label:Open details for Code");
  await app.waitFor("text:Resident memory");
  await app.press("id:sort-name");
  await app.waitFor("label:Sort by Name, ascending");
  await app.press("id:sort-name");
  await app.waitFor("label:Sort by Name, descending");
});

test("the More menu reaches workspace and device pages", async () => {
  const app = await open("../App.tsx", { ...desktop, modules: nativeModule() });
  await app.waitFor("id:page-overview");
  await app.press("id:titlebar-more");
  await app.press("id:menu-containers");
  await app.waitFor("id:page-containers");
  await app.waitFor("text:postgres:16");
  await app.waitFor("text:Up 2 hours (healthy)");
  await app.press("id:titlebar-more");
  await app.press("id:menu-projects");
  await app.waitFor("text:sysmon");
  await app.waitFor("text::5173");
  await app.press("label:Open details for sysmon");
  await app.waitFor("text:PID 4101");
  await app.press("id:titlebar-more");
  await app.press("id:menu-battery");
  await app.waitFor("text:4h 10m");
});

test("pausing holds the readings until resumed", async () => {
  const app = await open("../App.tsx", { ...desktop, modules: nativeModule(), clock: { time: 0 } });
  await app.waitFor("id:page-overview");
  await app.press("id:titlebar-cpu");
  await app.waitFor("text:LIVE HISTORY");
  await app.keyboard.press("space");
  await app.waitFor("text:PAUSED");
  await app.keyboard.press("space");
  await app.waitFor("text:LIVE HISTORY");
});

test("unavailable readings say so", async () => {
  const app = await open("../App.tsx", {
    ...desktop,
    modules: nativeModule({ overrides: { cpu: null, gpu: null, network: null, diskIo: null } }),
  });
  await app.waitFor("id:page-overview");
  await app.press("id:titlebar-gpu");
  await app.waitFor("text:The graphics driver does not expose utilization to this app. Hardware identity may still be available.");
  await app.press("id:titlebar-disk");
  await app.waitFor("text:Read Unavailable");
});

test("phones get tabs and a resource stack scoped to the app", async () => {
  const app = await open("../App.tsx", {
    platform: "ios",
    modules: nativeModule({ device: { name: "iOS · App scope", host: "This device", arch: "aarch64", scope: "ios" } }),
  });
  await app.waitFor("text:App CPU");
  await app.press("id:card-memory");
  await app.waitFor("id:page-memory");
  expect(app.shows("id:app-table")).toBe(false);
  await app.back();
  await app.waitFor("text:App CPU");
});
