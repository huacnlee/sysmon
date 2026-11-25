// The app's store: the latest reading, its history, the optional providers
// and whether sampling is paused. Created once by the App; pages read it from
// context.
import { batch, cleanup, context, signal, untracked, type Accessor } from "nsh";
import {
  emptyExtras,
  emptyHistory,
  record,
  type Device,
  type Extras,
  type History,
  type Reading,
  type Source,
} from "./model";

/** Core readings refresh this often. */
export const SAMPLE_MS = 2000;
/** Battery, audio, Bluetooth and Docker refresh this often. */
export const EXTRAS_MS = 30000;

export interface Monitor {
  /** The latest reading; `null` until the first one arrives. */
  latest: Accessor<Reading | null>;
  history: Accessor<History>;
  extras: Accessor<Extras>;
  /** Why the last reading failed, or `null`. */
  failure: Accessor<string | null>;
  paused: Accessor<boolean>;
  setPaused(paused: boolean): void;
  device: Device;
  uptime(): number | null;
}

/** Starts sampling `source`; it stops when the calling component goes. */
export function createMonitor(source: Source): Monitor {
  const latest = signal<Reading | null>(null);
  const history = signal<History>(emptyHistory);
  const extras = signal<Extras>(emptyExtras);
  const failure = signal<string | null>(null);
  const paused = signal(false);
  let alive = true;
  let sampling = false;
  let collecting = false;

  // Readings that arrive while paused are dropped: pause freezes the screen.
  async function sample() {
    if (sampling || untracked(paused)) return;
    sampling = true;
    try {
      const next = await source.sample();
      if (!alive || untracked(paused)) return;
      batch(() => {
        latest.set(next);
        history.update((h) => record(h, next));
        failure.set(null);
      });
    } catch (error) {
      if (alive) failure.set(error instanceof Error ? error.message : String(error));
    } finally {
      sampling = false;
    }
  }

  async function collect() {
    if (collecting || untracked(paused)) return;
    collecting = true;
    try {
      const next = await source.extras();
      if (alive && !untracked(paused)) extras.set(next);
    } catch {
      // Optional providers stay as they were; their status strings explain gaps.
    } finally {
      collecting = false;
    }
  }

  void sample();
  void collect();
  const samples = setInterval(() => void sample(), SAMPLE_MS);
  const collections = setInterval(() => void collect(), EXTRAS_MS);
  cleanup(() => {
    alive = false;
    clearInterval(samples);
    clearInterval(collections);
  });

  return {
    latest,
    history,
    extras,
    failure,
    paused,
    setPaused(value) {
      paused.set(value);
      if (!value) void sample();
    },
    device: source.device(),
    uptime: () => source.uptime(),
  };
}

export const MonitorContext = context<Monitor | null>(null);

/** The app's monitor, from the nearest provider. */
export function useMonitor(): Monitor {
  const monitor = MonitorContext();
  if (!monitor) throw new Error("useMonitor needs a MonitorContext.Provider");
  return monitor;
}
