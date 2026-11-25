// sysmon's styled layer over `nsh/base`, carrying the Rust app's design:
// raised panels with a lit top edge and a soft shadow, 11 pt muted labels,
// 13 pt semibold tinted titles, tinted icon tiles and chips, gradient meters,
// and the shared tooltip. Pages compose these instead of styling views.
import { Text, token, View, type JSX } from "nsh";
import { Tooltip } from "nsh/base";
import { Icon } from "nsh-icon";
import { BG, CHIP, GRADIENT, SECTIONS, TEXT, TILE, TRACK, type SectionId, type Tint } from "./sections";

export { Enter, INTERACTIVE, Metric } from "./motion";

/** The raised surface every card shares: a static native effect, so live
 * readings never rebuild it. */
export const PANEL = "rounded-(--radius) border border-border bg-card";
export function panelStyle() {
  return {
    boxShadow: `0 2px 10px -6px ${token("panel-shadow")}, inset 0 1px 0 ${token("panel-highlight")}`,
  };
}

/** The tooltip every hint in the app uses. */
export function Hint(props: { text: string }) {
  return (
    <Tooltip class="rounded-[5px] bg-tip px-2 py-[5px]">
      <Text class="text-xs text-tip-foreground">{props.text}</Text>
    </Tooltip>
  );
}

/** A raised region. */
export function Panel(props: { class?: string; testID?: string; children?: JSX.Element }) {
  return (
    <View testID={props.testID} class={`${PANEL} ${props.class ?? ""}`} style={panelStyle()}>
      {props.children}
    </View>
  );
}

/** A section's line icon, in its tint unless `class` gives another color. */
export function SectionIcon(props: { section: SectionId; size?: number; class?: string }) {
  return <Icon icon={SECTIONS[props.section].icon} size={props.size ?? 16} class={props.class ?? TEXT[props.section]} />;
}

/** A 26-point tinted tile for card and row headings. */
export function IconTile(props: { section: SectionId }) {
  return (
    <View class={`size-[26px] shrink-0 items-center justify-center rounded-[7px] ${TILE[props.section]}`}>
      <SectionIcon section={props.section} />
    </View>
  );
}

/** A 13 pt semibold title, tinted by its section, or in the foreground. */
export function Title(props: { tint?: Tint; class?: string; children: JSX.Element }) {
  return (
    <Text role="header" class={`text-[13px] font-semibold ${props.tint ? TEXT[props.tint] : "text-foreground"} ${props.class ?? ""}`}>
      {props.children}
    </Text>
  );
}

/** 11 pt muted text: notes, captions, metadata. */
export function Label(props: { class?: string; children: JSX.Element }) {
  return <Text class={`text-[11px] text-muted-foreground ${props.class ?? ""}`}>{props.children}</Text>;
}

/** A label and its 12 pt value on one line. */
export function Row(props: { label: string; value: string; testID?: string }) {
  return (
    <View testID={props.testID} class="flex-row items-center justify-between gap-2">
      <Label class="shrink-0">{props.label}</Label>
      <Text class="min-w-0 shrink truncate text-xs tabular-nums text-foreground">{props.value}</Text>
    </View>
  );
}

/** Status colors that are not a section's. */
export type Status = "active" | "healthy" | "warning" | "danger" | "muted";

const STATUS_TEXT: Record<Status, string> = {
  active: "text-active",
  healthy: "text-healthy",
  warning: "text-warning",
  danger: "text-danger",
  muted: "text-muted-foreground",
};
const STATUS_CHIP: Record<Status, string> = {
  active: "bg-active/12",
  healthy: "bg-healthy/12",
  warning: "bg-warning/12",
  danger: "bg-danger/12",
  muted: "bg-muted-foreground/12",
};

/** A short tinted state or tag: a port, a status, a process id. */
export function Chip(props: { tint?: Tint; status?: Status; children: JSX.Element }) {
  const text = () => (props.tint ? TEXT[props.tint] : STATUS_TEXT[props.status ?? "muted"]);
  const wash = () => (props.tint ? CHIP[props.tint] : STATUS_CHIP[props.status ?? "muted"]);
  return (
    <View class={`rounded-(--radius) px-2 py-1 ${wash()}`}>
      <Text class={`text-xs font-medium ${text()}`}>{props.children}</Text>
    </View>
  );
}

/** A summary card: what it counts, the value in 18 pt, what it means. Below
 * 900 points two share a row. */
export function Stat(props: { title: string; value: string; note: string; tint: Tint; compact?: boolean; testID?: string }) {
  return (
    <View
      testID={props.testID}
      class={`${PANEL} flex-1 gap-1.5 p-2.5 ${props.compact ? "min-w-[40%]" : "min-w-[190px]"}`}
      style={panelStyle()}
    >
      <Title tint={props.tint}>{props.title}</Title>
      <Text class="truncate text-lg font-semibold tabular-nums text-foreground">{props.value}</Text>
      <Label>{props.note}</Label>
    </View>
  );
}

/** A gradient meter, `fraction` of 0–1, on a track of its tint. Its width
 * changes in a native transition. */
export function Meter(props: { fraction: number; tint: Tint; thick?: boolean; track?: string; class?: string }) {
  const width = () => {
    const f = Number.isFinite(props.fraction) ? Math.min(1, Math.max(0, props.fraction)) : 0;
    return `${Math.round(f * 1000) / 10}%` as `${number}%`;
  };
  const h = () => (props.thick ? "h-1.5" : "h-1");
  return (
    <View accessible={false} class={`w-full overflow-hidden rounded-full ${h()} ${props.track ?? TRACK[props.tint]} ${props.class ?? ""}`}>
      <View
        class={`rounded-full transition-all duration-200 ease-out motion-reduce:transition-none ${h()} ${GRADIENT[props.tint]}`}
        style={{ width: width() }}
      />
    </View>
  );
}

/** A five-point track at a fixed width, as container memory shows it;
 * `high` marks a value near its limit. */
export function Progress(props: { percent: number; tint: Tint; high?: boolean; width: number }) {
  const f = () => Math.min(100, Math.max(0, Number.isFinite(props.percent) ? props.percent : 0)) / 100;
  return (
    <View accessible={false} class={`h-[5px] rounded-full ${TRACK[props.tint]}`} style={{ width: props.width }}>
      <View class={`h-[5px] rounded-full ${props.high ? "bg-high" : BG[props.tint]}`} style={{ width: props.width * f() }} />
    </View>
  );
}
