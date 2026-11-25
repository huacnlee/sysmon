// Live history charts, drawn as the Rust app drew them: actual samples at
// their capture times, a restrained dotted grid, a gradient trace over a
// gradient wash, a pulsing endpoint, pause gaps, and a readout under the
// pointer. Values glide on native animated values; no JS runs per frame.
import { Canvas, computed, createAnimatedValue, effect, reduceMotion, signal, token, untracked, type AnimatedValue } from "nsh";
import { CAPACITY, rate, type Point } from "./model";
import { END, type Tint } from "./sections";

/** `percent` and `storage` span 0–100 (`memory` charts zoom to their
 * samples); `rate` is bytes per second from zero. */
export type ChartUnits = "percent" | "storage" | "rate";

/** Seconds of history a chart starts with: seven two-second intervals. */
export const MIN_HORIZON = 14;

/** Consecutive samples further apart than this are separate runs. */
const GAP = 6;

/** Ages (seconds before the newest sample) split where sampling paused. */
export function segments(ages: number[]): [number, number][] {
  if (ages.length === 0) return [];
  const runs: [number, number][] = [];
  let start = 0;
  for (let i = 1; i < ages.length; i++) {
    if (ages[i - 1]! - ages[i]! > GAP) {
      runs.push([start, i]);
      start = i;
    }
  }
  runs.push([start, ages.length]);
  return runs;
}

/** The value axis. Memory moves little, so its chart spans its samples with
 * a margin, and says so. Rates leave headroom above the peak. */
export function valueDomain(values: number[], secondary: number[], units: ChartUnits, memory: boolean): [number, number] {
  if (units === "rate") return [0, Math.max(1, Math.max(0, ...values, ...secondary) * 1.12)];
  if (!memory || values.length === 0) return [0, 100];
  const min = Math.min(100, ...values);
  const max = Math.max(0, ...values);
  const span = Math.min(100, Math.max(1, (max - min) * 1.3));
  const low = Math.min(100 - span, Math.max(0, (min + max) * 0.5 - span * 0.5));
  return [low, low + span];
}

export function timeLabel(seconds: number): string {
  const n = Math.round(seconds);
  if (n === 0) return "now";
  if (n % 60 === 0) return `−${n / 60}m`;
  if (n >= 60) return `−${Math.floor(n / 60)}m${n % 60}s`;
  return `−${n}s`;
}

function axisValue(value: number, span: number): string {
  return span <= 2 ? value.toFixed(2) : value.toFixed(0);
}

/** `#rrggbbaa` at `alpha` of a token's color. */
function fade(color: string, alpha: number): string {
  const a = Math.round(Math.min(1, Math.max(0, alpha)) * 255)
    .toString(16)
    .padStart(2, "0");
  return `${color.slice(0, 7)}${a}`;
}

function observations(points: Point[], units: ChartUnits): Point[] {
  return points
    .slice(-CAPACITY)
    .filter((p) => Number.isFinite(p.v))
    .map((p) => ({ t: p.t, v: units === "rate" ? Math.max(0, p.v) : Math.min(100, Math.max(0, p.v)) }));
}

/** Slot targets: the samples right-aligned in `CAPACITY` slots. */
function targets(points: Point[]): number[] {
  const out = new Array<number>(CAPACITY).fill(0);
  const start = CAPACITY - points.length;
  points.forEach((p, i) => (out[start + i] = p.v));
  return out;
}

export interface LiveChartProps {
  points: Point[];
  tint: Tint;
  units: ChartUnits;
  /** Memory: a windowed vertical range, labelled as such. */
  memory?: boolean;
  /** Upload beside download, sharing the scales. */
  secondary?: Point[];
  secondaryTint?: Tint;
  /** A card's sparkline: smaller type, no value axis. */
  compact?: boolean;
  /** Fill the height the parent gives it; otherwise a default height. */
  fit?: boolean;
  paused?: boolean;
  class?: string;
}

export function LiveChart(props: LiveChartProps) {
  const reduced = () => reduceMotion();
  const samples = computed(() => observations(props.points, props.units));
  const second = computed(() => observations(props.secondary ?? [], props.units));
  const domain = computed(() =>
    valueDomain(
      samples().map((p) => p.v),
      second().map((p) => p.v),
      props.units,
      props.memory ?? false,
    ),
  );

  // Native slots: each sample's height glides to its new place.
  const initial = untracked(samples);
  const initialSecond = untracked(second);
  const initialDomain = untracked(domain);
  const slots: AnimatedValue[] = targets(initial).map((v) => createAnimatedValue(v));
  const secondSlots: AnimatedValue[] = targets(initialSecond).map((v) => createAnimatedValue(v));
  const low = createAnimatedValue(initialDomain[0]);
  const high = createAnimatedValue(initialDomain[1]);
  const beat = createAnimatedValue(1);
  let previous = targets(initial);
  let previousSecond = targets(initialSecond);
  let previousDomain = initialDomain;
  let previousCount = initial.length;
  let previousLast = initial[initial.length - 1]?.t;

  effect(() => {
    const next = samples();
    const nextSecond = second();
    const [lo, hi] = domain();
    const still = reduced();
    const glide = { duration: 520, easing: "easeOut" as const };
    if (still) {
      low.set(lo);
      high.set(hi);
    } else if (previousDomain[0] !== lo || previousDomain[1] !== hi) {
      void low.timing(lo, glide);
      void high.timing(hi, glide);
    }
    previousDomain = [lo, hi];
    const t = targets(next);
    for (let i = 0; i < CAPACITY; i++) {
      if (still) slots[i]!.set(t[i]!);
      else if (previous[i] !== t[i]) void slots[i]!.timing(t[i]!, glide);
    }
    previous = t;
    const s = targets(nextSecond);
    for (let i = 0; i < CAPACITY; i++) {
      if (still) secondSlots[i]!.set(s[i]!);
      else if (previousSecond[i] !== s[i]) void secondSlots[i]!.timing(s[i]!, glide);
    }
    previousSecond = s;
    const last = next[next.length - 1]?.t;
    if (!still && next.length > 0 && (last !== previousLast || next.length !== previousCount)) {
      beat.set(0);
      void beat.timing(1, { duration: 900, easing: "easeOut" });
    }
    previousLast = last;
    previousCount = next.length;
  });

  const width = signal(0);
  const cursor = signal<number | null>(null);
  const left = () => (props.compact ? 10 : props.units === "rate" ? 64 : 31);

  const ages = () => {
    const s = samples();
    const end = s[s.length - 1]?.t ?? 0;
    return s.map((p) => end - p.t);
  };
  const horizon = () => Math.max(MIN_HORIZON, ages()[0] ?? 0);

  function select(x: number) {
    const w = width();
    if (w <= 0) return;
    const fraction = Math.min(1, Math.max(0, (x - left()) / Math.max(1, w - 10 - left())));
    const age = (1 - fraction) * horizon();
    let best: number | null = null;
    ages().forEach((a, i) => {
      if (best == null || Math.abs(a - age) < Math.abs(ages()[best]! - age)) best = i;
    });
    cursor.set(best);
  }

  return (
    <Canvas
      testID={`chart-${props.memory ? "memory" : "line"}-${props.compact ? "compact" : "detail"}`}
      accessibilityLabel="Live resource history chart"
      class={`w-full ${props.fit ? "min-h-0 flex-1" : ""} ${props.class ?? ""}`}
      style={props.fit ? undefined : { height: props.compact ? 94 : 176 }}
      onLayout={({ bounds }) => width.set(bounds.size.width)}
      onMouseMove={(e) => select(e.position.x)}
      onMouseLeave={() => cursor.set(null)}
      onTouchStart={(e) => select(e.locationX)}
      onTouchMove={(e) => select(e.locationX)}
      onTouchEnd={() => cursor.set(null)}
      draw={(cx, size) => {
        const observed = samples();
        const count = observed.length;
        const secondary = second();
        const a = ages();
        const span = horizon();
        const [domainMin, domainMax] = domain();
        const domainSpan = domainMax - domainMin;
        const compact = props.compact ?? false;
        const w = size.width;
        const h = size.height;
        const l = left();
        const right = Math.max(l + 2, w - 10);
        const rows = props.units === "rate" && props.secondary && h >= 84 ? 2 : 1;
        const tipSize = compact ? 9 : 10;
        const tipLine = tipSize + 3;
        const axisRoom = h < 84 ? 19 : 25;
        const bottom = Math.max(0, h - axisRoom);
        const top = Math.min(Math.max(tipLine * rows + 16, 28), Math.max(0, bottom - 16));
        const plotH = bottom - top;
        const plotW = right - l;
        const accent = token(props.tint);
        const endpoint = token(END[props.tint]);
        const muted = token("plot-label");
        const font = compact ? "9px system-ui" : "10px system-ui";
        cx.font = font;

        // A restrained dotted grid leaves the readings and trace dominant.
        for (const fraction of [0, 0.5, 1]) {
          const y = top + plotH * fraction;
          cx.strokeStyle = token(fraction === 1 ? "plot-base" : "plot-grid");
          cx.lineWidth = 0.7;
          cx.setLineDash([1, 4]);
          cx.beginPath();
          cx.moveTo(l, y);
          cx.lineTo(right, y);
          cx.stroke();
          if (!compact) {
            const value = domainMax - domainSpan * fraction;
            cx.fillStyle = muted;
            cx.textAlign = "left";
            cx.fillText(props.units === "rate" ? rate(value) : axisValue(value, domainSpan), 2, y + 3);
          }
        }
        cx.setLineDash([]);
        cx.fillStyle = muted;
        cx.textAlign = "left";
        cx.fillText(timeLabel(span), l, h - 3);
        cx.textAlign = "right";
        cx.fillText(props.paused ? "paused" : "latest", right, h - 3);
        if (!compact) {
          cx.textAlign = "center";
          cx.fillText(timeLabel(span * 0.5), l + plotW * 0.5, h - 3);
        }
        cx.textAlign = "left";
        if (compact && props.memory && count > 0 && cursor() == null) {
          cx.font = "8px system-ui";
          cx.fillText(`range ${axisValue(domainMin, domainSpan)}–${axisValue(domainMax, domainSpan)}%`, l + 2, 7);
          cx.font = font;
        }
        if (count === 0) {
          cx.fillStyle = token("plot-empty");
          cx.font = compact ? "9px system-ui" : "11px system-ui";
          cx.fillText(compact ? "No history" : "No samples available", l + 4, top + plotH * 0.6);
          return;
        }

        const first = CAPACITY - count;
        const yOf = (slot: AnimatedValue) =>
          slot
            .sub(low)
            .div(high.sub(low))
            .interpolate({ inputRange: [0, 1], outputRange: [bottom, top], extrapolate: "extend" });
        const xOf = (age: number) => right - (age / Math.max(0.001, span)) * (right - l);
        const xy = a.map((age, i) => ({ x: xOf(age), y: yOf(slots[first + i]!) }));

        const fill = cx.createLinearGradient(l, top, right, bottom);
        fill.addColorStop(0, fade(accent, 0.24));
        fill.addColorStop(0.55, fade(endpoint, 0.1));
        fill.addColorStop(1, fade(endpoint, 0.012));
        const stroke = cx.createLinearGradient(l, top, right, top);
        stroke.addColorStop(0, accent);
        stroke.addColorStop(1, endpoint);
        for (const [start, end] of segments(a)) {
          const run = xy.slice(start, end);
          cx.fillStyle = fill;
          cx.beginPath();
          cx.moveTo(run[0]!.x, bottom);
          for (const p of run) cx.lineTo(p.x, p.y);
          cx.lineTo(run[run.length - 1]!.x, bottom);
          cx.closePath();
          cx.fill();
          cx.strokeStyle = stroke;
          cx.lineWidth = 2.75;
          cx.lineCap = "round";
          cx.lineJoin = "round";
          cx.beginPath();
          run.forEach((p, i) => (i === 0 ? cx.moveTo(p.x, p.y) : cx.lineTo(p.x, p.y)));
          cx.stroke();
        }
        const last = xy[xy.length - 1]!;
        // One pulse per new sample, settling to stillness.
        if (!reduced()) {
          cx.save();
          cx.globalAlpha = beat.interpolate({ inputRange: [0, 1], outputRange: [0.32, 0] });
          cx.strokeStyle = endpoint;
          cx.lineWidth = 1;
          cx.beginPath();
          cx.arc(last.x, last.y, beat.interpolate({ inputRange: [0, 1], outputRange: [3, h < 84 ? 5 : 9] }), 0, Math.PI * 2);
          cx.stroke();
          cx.restore();
        }
        cx.fillStyle = token("plot-hole");
        cx.beginPath();
        cx.arc(last.x, last.y, 4, 0, Math.PI * 2);
        cx.fill();
        cx.fillStyle = endpoint;
        cx.beginPath();
        cx.arc(last.x, last.y, 3, 0, Math.PI * 2);
        cx.fill();

        const secondTint = props.secondaryTint ?? "upload";
        if (props.secondary) {
          const n = Math.min(secondary.length, count);
          if (n > 0) {
            const secondStroke = cx.createLinearGradient(l, top, right, top);
            secondStroke.addColorStop(0, token(secondTint));
            secondStroke.addColorStop(1, token(END[secondTint]));
            cx.strokeStyle = secondStroke;
            cx.lineWidth = 2.5;
            cx.lineCap = "round";
            cx.lineJoin = "round";
            cx.beginPath();
            const secondFirst = CAPACITY - secondary.length;
            const offset = count - n;
            for (const [start, end] of segments(a.slice(offset))) {
              for (let at = start + offset; at < end + offset; at++) {
                const index = secondary.length + at - count;
                const x = xOf(a[at]!);
                const y = yOf(secondSlots[secondFirst + index]!);
                if (at === start + offset) cx.moveTo(x, y);
                else cx.lineTo(x, y);
              }
            }
            cx.stroke();
            const endY = yOf(secondSlots[CAPACITY - 1]!);
            cx.fillStyle = token("plot-hole");
            cx.beginPath();
            cx.arc(right, endY, 4, 0, Math.PI * 2);
            cx.fill();
            cx.fillStyle = token(END[secondTint]);
            cx.beginPath();
            cx.arc(right, endY, 2.8, 0, Math.PI * 2);
            cx.fill();
          }
        }

        const selected = cursor();
        if (selected != null && selected < count) {
          const p = xy[selected]!;
          cx.strokeStyle = fade(accent, 0.55);
          cx.lineWidth = 0.8;
          cx.setLineDash([2, 3]);
          cx.beginPath();
          cx.moveTo(p.x, top);
          cx.lineTo(p.x, bottom);
          cx.stroke();
          cx.setLineDash([]);
          cx.fillStyle = token("plot-hole");
          cx.beginPath();
          cx.arc(p.x, p.y, 4.5, 0, Math.PI * 2);
          cx.fill();
          cx.strokeStyle = accent;
          cx.lineWidth = 1.5;
          cx.beginPath();
          cx.arc(p.x, p.y, 3, 0, Math.PI * 2);
          cx.stroke();
          const value = observed[selected]!.v;
          let text: string;
          if (props.units === "rate") {
            const tx = props.secondary ? secondary[secondary.length - (count - selected)] : undefined;
            text = tx ? `↓ ${rate(value)}  ↑ ${rate(tx.v)}` : props.secondary ? `↓ ${rate(value)}` : rate(value);
          } else if (props.units === "storage") text = `${value.toFixed(1)}% used`;
          else if (props.memory) text = `${value.toFixed(1)}% memory`;
          else text = `${value.toFixed(1)}%`;
          const lines = rows > 1 ? text.split("  ") : [text];
          lines[0] += ` · −${Math.round(a[selected]!)}s`;
          cx.font = font;
          const room = Math.max(1, w - 14);
          const widest = Math.max(...lines.map((line) => cx.measureText(line).width));
          const sizePt = Math.max(7, Math.min(tipSize, (tipSize * room) / Math.max(1, widest)));
          const tipFont = `${sizePt}px system-ui`;
          cx.font = tipFont;
          const lineHeight = sizePt + 3;
          const boxW = Math.min(Math.max(...lines.map((line) => cx.measureText(line).width)) + 8, Math.max(0, w - 4));
          const boxH = lineHeight * lines.length + 4;
          const boxX = Math.min(Math.max(2, p.x - boxW * 0.5), Math.max(2, w - boxW - 2));
          cx.fillStyle = token("tip");
          cx.beginPath();
          cx.roundRect(boxX, 2, boxW, boxH, 4);
          cx.fill();
          cx.fillStyle = token("tip-foreground");
          cx.textBaseline = "top";
          lines.forEach((line, i) => cx.fillText(line, boxX + 4, 4 + i * lineHeight, boxW - 8));
          cx.textBaseline = "alphabetic";
        } else if (!compact) {
          cx.font = "10px system-ui";
          if (props.units === "rate" && props.secondary) {
            let rx = "↓ DOWNLOAD";
            let tx = "↑ UPLOAD";
            const gap = 12;
            if (cx.measureText(rx).width + gap + cx.measureText(tx).width > Math.max(0, w - 16)) {
              rx = "↓ RX";
              tx = "↑ TX";
            }
            const rxWidth = cx.measureText(rx).width;
            const total = rxWidth + gap + cx.measureText(tx).width;
            const start = total <= plotW - 6 ? l + 3 : 8;
            cx.fillStyle = accent;
            cx.fillText(rx, start, top - 6);
            cx.fillStyle = token(END[secondTint]);
            cx.fillText(tx, start + rxWidth + gap, top - 6);
          } else {
            cx.fillStyle = accent;
            cx.fillText(
              props.units === "rate" ? "TRANSFER RATE" : props.memory ? "WINDOWED MEMORY %" : props.units === "storage" ? "STORAGE USED %" : "UTILIZATION %",
              l + 3,
              top - 6,
            );
          }
        }
      }}
    />
  );
}
