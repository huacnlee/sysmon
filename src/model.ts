// The data sysmon shows, as `sysmon-native` reports it, and the pure rules
// that turn readings into history and text. Nothing here touches the UI.

/** A mounted volume, in bytes. */
export interface Volume {
  name: string;
  total: number;
  free: number;
}

/** Transfer rates in bytes per second. */
export interface Interface {
  name: string;
  rx: number;
  tx: number;
}

/** An application's processes, summed. CPU 100 is one core. */
export interface AppUsage {
  id: string;
  name: string;
  cpu: number;
  memory: number;
  processes: number;
}

/** A development project or coding assistant found among local processes. */
export interface Workload {
  name: string;
  detail: string;
  memory: number;
  cpu: number;
  pid: number;
  pids: number[];
  startedSeconds: number;
  runningProcesses: number;
}

/** One reading. `null` means unavailable, never a measured zero. */
export interface Reading {
  /** Seconds since the sampler started. */
  at: number;
  cpu: number | null;
  cores: number;
  memory: number | null;
  total: number;
  disk: Volume | null;
  diskIo: { read: number; write: number } | null;
  network: Interface | null;
  gpu: number | null;
  volumes: Volume[];
  interfaces: Interface[];
  apps: AppUsage[];
  workloads: { projects: Workload[]; agents: Workload[] };
}

export interface Battery {
  percent: number;
  status: string;
  remaining: string | null;
  health: number | null;
  cycles: number | null;
  powerW: number | null;
}

export interface Container {
  name: string;
  image: string;
  memory: string;
  cpu: string;
  ports: string;
  project: string;
  status: string;
  memoryUsed: number;
  memoryLimit: number;
  cpuPercent: number;
  networkRx: number;
  networkTx: number;
}

/** The slower, optional providers. Empty status strings mean "not read yet". */
export interface Extras {
  battery: Battery | null;
  batteryStatus: string;
  deviceModel: string | null;
  osVersion: string | null;
  gpu: string | null;
  sound: string | null;
  containers: Container[];
  containerStatus: string;
  bluetooth: string[];
  bluetoothStatus: string;
  /** `[pid, port]` pairs. */
  listeningPorts: [number, string][];
}

/** What the app may observe: the whole desktop, or only itself on a phone. */
export type Scope = "desktop" | "ios" | "android";

export interface Device {
  name: string;
  host: string;
  arch: string;
  scope: Scope;
}

/** Where readings come from: `sysmon-native`, or a test's stand-in. */
export interface Source {
  sample(): Promise<Reading>;
  extras(): Promise<Extras>;
  device(): Device;
  uptime(): number | null;
}

export const emptyExtras: Extras = {
  battery: null,
  batteryStatus: "",
  deviceModel: null,
  osVersion: null,
  gpu: null,
  sound: null,
  containers: [],
  containerStatus: "",
  bluetooth: [],
  bluetoothStatus: "",
  listeningPorts: [],
};

// ---------------------------------------------------------------- history

/** Samples kept per series: two minutes at the two-second interval. */
export const CAPACITY = 60;

/** One point of a series: when it was read, and its value. */
export interface Point {
  t: number;
  v: number;
}

export type SeriesKey = "cpu" | "memory" | "disk" | "download" | "upload" | "gpu";

export type History = Record<SeriesKey, Point[]>;

export const emptyHistory: History = {
  cpu: [],
  memory: [],
  disk: [],
  download: [],
  upload: [],
  gpu: [],
};

function push(series: Point[], t: number, value: number | null | undefined): Point[] {
  if (value == null || !Number.isFinite(value)) return series;
  const next = series.length >= CAPACITY ? series.slice(series.length - CAPACITY + 1) : series.slice();
  next.push({ t, v: value });
  return next;
}

/** Adds a reading's values to the history. An unavailable value adds no
 * point, so a gap stays a gap instead of becoming a zero. */
export function record(history: History, r: Reading): History {
  return {
    cpu: push(history.cpu, r.at, r.cpu),
    memory: push(history.memory, r.at, r.memory != null && r.total > 0 ? (r.memory / r.total) * 100 : null),
    disk: push(history.disk, r.at, r.disk && r.disk.total > 0 ? 100 - (r.disk.free / r.disk.total) * 100 : null),
    download: push(history.download, r.at, r.network?.rx),
    upload: push(history.upload, r.at, r.network?.tx),
    gpu: push(history.gpu, r.at, r.gpu),
  };
}

export function average(series: Point[]): number | null {
  if (series.length === 0) return null;
  return series.reduce((sum, p) => sum + p.v, 0) / series.length;
}

export function peak(series: Point[]): number | null {
  if (series.length === 0) return null;
  return series.reduce((max, p) => Math.max(max, p.v), 0);
}

// ---------------------------------------------------------------- text

/** Shown for any reading the device does not give. */
export const UNAVAILABLE = "Unavailable";

/** `value` formatted, or "Unavailable" when it is missing. */
export function reading<T>(value: T | null | undefined, format: (value: T) => string): string {
  return value == null ? UNAVAILABLE : format(value);
}

/** Bytes, in decimal units: "512 MB", "3.25 GB". */
export function bytes(n: number): string {
  return n < 1e9 ? `${(n / 1e6).toFixed(0)} MB` : `${(n / 1e9).toFixed(2)} GB`;
}

/** Bytes per second: "999 B/s", "1.5 kB/s", "2.0 MB/s". */
export function rate(n: number): string {
  if (n < 1e3) return `${Math.round(n)} B/s`;
  if (n < 1e6) return `${(n / 1e3).toFixed(1)} kB/s`;
  if (n < 1e9) return `${(n / 1e6).toFixed(1)} MB/s`;
  return `${(n / 1e9).toFixed(1)} GB/s`;
}

export function percent(n: number, digits = 0): string {
  return `${n.toFixed(digits)} %`;
}

/** "3d 4h", "2h 5m", "7m". */
export function duration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  if (s >= 86400) return `${Math.floor(s / 86400)}d ${Math.floor((s % 86400) / 3600)}h`;
  if (s >= 3600) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  return `${Math.floor(s / 60)}m`;
}

const UNITS = new Set(["%", "GB", "MB", "B/s", "kB/s", "MB/s", "GB/s", "W"]);

/** "18.40 GB" → ["18.40", "GB"]; a value without a known unit stays whole. */
export function splitUnit(text: string): [string, string] {
  const at = text.lastIndexOf(" ");
  if (at > 0 && UNITS.has(text.slice(at + 1))) return [text.slice(0, at), text.slice(at + 1)];
  return [text, ""];
}

export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

// ---------------------------------------------------------------- apps

export type AppOrder = "cpu" | "memory" | "name";

/** Apps matching `query` (case-insensitive), sorted by `order`. Ties fall back
 * to the name, then the id, so rows never swap places between readings. */
export function orderApps(apps: AppUsage[], query: string, order: AppOrder, descending: boolean): AppUsage[] {
  const q = query.trim().toLowerCase();
  const byName = (a: AppUsage, b: AppUsage) =>
    a.name.toLowerCase().localeCompare(b.name.toLowerCase()) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
  return apps
    .filter((app) => app.name.toLowerCase().includes(q))
    .sort((a, b) => {
      const primary = order === "cpu" ? a.cpu - b.cpu : order === "memory" ? a.memory - b.memory : byName(a, b);
      return (descending ? -primary : primary) || byName(a, b);
    });
}

/** Up to two initials for an app's avatar. */
export function initials(name: string): string {
  const value = name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => word[0]!.toUpperCase())
    .join("");
  return value || "A";
}

/** Theme tokens an app's avatar may take, chosen by its name so it stays put. */
export const AVATAR_TOKENS = ["cpu", "memory", "network", "gpu", "disk", "containers"] as const;

export function avatarToken(name: string): (typeof AVATAR_TOKENS)[number] {
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (Math.imul(hash, 31) + name.charCodeAt(i)) >>> 0;
  return AVATAR_TOKENS[hash % AVATAR_TOKENS.length]!;
}

// ---------------------------------------------------------------- containers

export type Tone = "healthy" | "warning" | "danger" | "muted";

/** Only healthy running containers read as healthy. */
export function containerTone(status: string): Tone {
  const s = status.toLowerCase();
  if (s.includes("unhealthy") || s.startsWith("dead")) return "danger";
  if (s.startsWith("restarting") || s.includes("paused") || s.includes("health: starting")) return "warning";
  if (s.startsWith("up") || s.startsWith("running")) return "healthy";
  return "muted";
}

export function reports(value: string): boolean {
  return value !== "" && value !== UNAVAILABLE;
}

/** Containers grouped by Compose project, standalone ones last. */
export function groupContainers(containers: Container[]): { name: string; containers: Container[] }[] {
  const groups = new Map<string, Container[]>();
  for (const c of containers) {
    const name = c.project || "Standalone";
    groups.set(name, [...(groups.get(name) ?? []), c]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => (a === "Standalone" ? 1 : b === "Standalone" ? -1 : a.localeCompare(b)))
    .map(([name, containers]) => ({ name, containers }));
}

/** The ports a workload's processes listen on. */
export function portsOf(workload: Workload, ports: [number, string][]): string[] {
  return ports.filter(([pid]) => workload.pids.includes(pid)).map(([, port]) => port);
}
