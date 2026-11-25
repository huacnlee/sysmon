// Readings for tests: a busy desktop, as `sysmon-native` reports it.
import type { AppUsage, Device, Extras, Reading } from "../src/model";

export const device: Device = { name: "Arch Linux", host: "workstation", arch: "x86_64", scope: "desktop" };

export function app(name: string, cpu: number, memory: number, processes = 2): AppUsage {
  return { id: `app-${name}`, name, cpu, memory, processes };
}

export function reading(at: number, overrides: Partial<Reading> = {}): Reading {
  return {
    at,
    cpu: 20 + (at % 7),
    cores: 8,
    memory: 6e9,
    total: 16e9,
    disk: { name: "/", total: 500e9, free: 200e9 },
    diskIo: { read: 1500, write: 2_500_000 },
    network: { name: "All interfaces", rx: 12_000, tx: 3_000 },
    gpu: 12,
    volumes: [
      { name: "/", total: 500e9, free: 200e9 },
      { name: "/home", total: 1000e9, free: 400e9 },
    ],
    interfaces: [{ name: "wlan0", rx: 12_000, tx: 3_000 }],
    apps: [app("Firefox", 31.5, 2.1e9, 12), app("Code", 12.25, 1.4e9, 8), app("Terminal", 0.5, 80e6, 1), app("Slack", 4, 900e6, 5)],
    workloads: {
      projects: [
        {
          name: "sysmon",
          detail: "bun dev server",
          memory: 300e6,
          cpu: 2.5,
          pid: 4100,
          pids: [4100, 4101],
          startedSeconds: 3900,
          runningProcesses: 2,
        },
      ],
      agents: [],
    },
    ...overrides,
  };
}

export const extras: Extras = {
  battery: { percent: 82, status: "Discharging", remaining: "4h 10m", health: 91, cycles: 120, powerW: 9.5 },
  batteryStatus: "",
  deviceModel: null,
  osVersion: null,
  gpu: "Radeon 780M",
  sound: "Speakers",
  containers: [
    {
      name: "db",
      image: "postgres:16",
      memory: "120 MB",
      cpu: "0.4%",
      ports: "5432",
      project: "shop",
      status: "Up 2 hours (healthy)",
      memoryUsed: 120e6,
      memoryLimit: 2e9,
      cpuPercent: 0.4,
      networkRx: 1e6,
      networkTx: 2e6,
    },
  ],
  containerStatus: "Docker engine connected",
  bluetooth: ["Keyboard"],
  bluetoothStatus: "1 connected device",
  listeningPorts: [[4100, ":5173"]],
};

/** `open`'s stand-ins for `sysmon-native`: readings two seconds apart. */
export function nativeModule(options: { extras?: Extras; device?: Device; overrides?: Partial<Reading> } = {}) {
  let at = 0;
  return {
    "sysmon-native": {
      "Sysmon.sample": () => JSON.stringify(reading((at += 2), options.overrides)),
      "Sysmon.extras": () => JSON.stringify(options.extras ?? extras),
      "Sysmon.device": JSON.stringify(options.device ?? device),
      "Sysmon.uptime": 7200,
    },
  };
}
