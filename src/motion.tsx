// Small, native-driven motions, as the Rust app had them: card entrances,
// pressed feedback, and the odometer whose digits roll on native animated
// values. Sampling never starts a JS frame loop.
import {
  computed,
  createAnimatedValue,
  effect,
  reduceMotion,
  signal,
  Text,
  untracked,
  View,
  type Accessor,
  type JSX,
} from "nsh";
import { splitUnit } from "./model";

/** Fades and rises its content in once, after `delay` milliseconds. */
export function Enter(props: { delay?: number; class?: string; testID?: string; children?: JSX.Element }) {
  const reduced = untracked(reduceMotion);
  const progress = createAnimatedValue(reduced ? 1 : 0);
  if (!reduced) void progress.timing(1, { duration: 360, delay: props.delay ?? 0, easing: "easeOut" });
  return (
    <View
      testID={props.testID}
      class={props.class}
      style={{
        opacity: progress,
        transform: [
          { translateY: progress.interpolate({ inputRange: [0, 1], outputRange: [12, 0] }) },
          { scale: progress.interpolate({ inputRange: [0, 1], outputRange: [0.985, 1] }) },
        ],
      }}
    >
      {props.children}
    </View>
  );
}

/** Pressable cards: a lighter edge on hover, a slight press-in. */
export const INTERACTIVE =
  "transition-all duration-150 ease-out hover:border-card-hover-edge active:scale-[0.982] motion-reduce:transition-none motion-reduce:active:scale-100";

interface ReelStyle {
  font: number;
  height: number;
  width: number;
  unit: number;
  compact: boolean;
}

const LARGE: ReelStyle = { font: 28, height: 34, width: 18, unit: 12, compact: false };
const SMALL: ReelStyle = { font: 12, height: 16, width: 8, unit: 10, compact: true };

/** "18.40" → "##.##"; `null` for anything that is not a plain number. */
export function numberShape(number: string): string | null {
  if (!number || !/\d/.test(number) || !/^[\d.+-]+$/.test(number)) return null;
  return number.replace(/\d/g, "#");
}

function digitAt(number: string, index: number): string {
  const ch = number[index];
  return ch && /\d/.test(ch) ? ch : "0";
}

/** The old and new glyphs, and the rows of the two-line strip they sit on:
 * every change moves one line, however far the value jumps. */
export function digitTransition(from: string, to: string, increasing: boolean): [string, number, number] {
  return increasing ? [`${from}\n${to}`, 0, 1] : [`${to}\n${from}`, 1, 0];
}

function Reel(props: { number: Accessor<string>; index: number; style: ReelStyle }) {
  const s = props.style;
  const first = untracked(props.number);
  let previous = first;
  const initial = digitAt(first, props.index);
  const pair = signal(`${initial}\n${initial}`);
  const position = createAnimatedValue(0);
  effect(() => {
    const next = props.number();
    const old = previous;
    previous = next;
    const from = digitAt(old, props.index);
    const to = digitAt(next, props.index);
    if (reduceMotion()) {
      pair.set(`${to}\n${to}`);
      position.set(0);
    } else if (from !== to) {
      const [digits, start, end] = digitTransition(from, to, Number(next) >= Number(old));
      // Text and offset change in the same update: the strip starts exactly
      // on the digit that was showing.
      pair.set(digits);
      position.set(-start * s.height);
      void position.timing(-end * s.height, { duration: 260, easing: "easeOut" });
    }
  });
  return (
    <View accessible={false} class="shrink-0 overflow-hidden" style={{ width: s.width, height: s.height }}>
      <Text
        accessible={false}
        class="shrink-0 text-center font-medium tabular-nums text-foreground"
        style={{
          width: s.width,
          height: s.height * 2,
          fontSize: s.font,
          lineHeight: s.height,
          transform: [{ translateY: position }],
        }}
      >
        {pair()}
      </Text>
    </View>
  );
}

/** A reading as an odometer: digits roll, while the decimal point, the sign
 * and the unit stay put. Text that is not a number shows as it is. */
export function Metric(props: { value: string; size?: "large" | "small"; testID?: string }) {
  const s = (props.size ?? "large") === "small" ? SMALL : LARGE;
  const number = computed(() => splitUnit(props.value)[0]);
  const unit = () => splitUnit(props.value)[1];
  const shape = computed(() => numberShape(number()));
  return (
    <View
      testID={props.testID}
      accessible
      accessibilityLabel={props.value}
      class="flex-row items-end"
      style={{ gap: s.compact ? 2 : 4, paddingVertical: s.compact ? 0 : 4 }}
    >
      {(() => {
        const current = shape();
        if (current == null)
          return (
            <Text
              accessible={false}
              class="font-medium text-foreground"
              style={{ fontSize: s.compact ? s.font : 16, lineHeight: s.height }}
            >
              {number()}
            </Text>
          );
        return (
          <View accessible={false} class="flex-row items-start" style={{ height: s.height }}>
            {current.split("").map((ch, index) =>
              ch === "#" ? (
                <Reel number={number} index={index} style={s} />
              ) : (
                <Text
                  accessible={false}
                  class="font-medium tabular-nums text-foreground"
                  style={{ fontSize: s.font, lineHeight: s.height }}
                >
                  {ch}
                </Text>
              ),
            )}
          </View>
        );
      })()}
      <Text accessible={false} class="text-muted-foreground" style={{ fontSize: s.unit, paddingBottom: s.compact ? 0 : 3 }}>
        {unit()}
      </Text>
    </View>
  );
}
