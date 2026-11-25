// CPU, memory, storage and network: a history chart, summary figures, and
// what the resource is spent on (applications, volumes or interfaces).
import { Scroll, Text, View, windowSize } from "nsh";
import { AppTable } from "../apps";
import { LiveChart } from "../chart";
import { average, bytes, peak, percent, plural, rate, reading, type Reading, type History } from "../model";
import { useMonitor } from "../monitor";
import { TEXT } from "../sections";
import { Enter, Label, Metric, PANEL, panelStyle, Row, Stat, Title } from "../ui";

export type ResourceId = "cpu" | "memory" | "disk" | "network";

interface Figure {
  title: string;
  value: string;
  note: string;
  /** Shown on phones too; the rest describe other apps, which phones hide. */
  mobile: boolean;
}

/** The headline reading of a resource. */
export function headline(id: ResourceId, r: Reading | null): string {
  switch (id) {
    case "cpu":
      return reading(r?.cpu, (v) => percent(v));
    case "memory":
      return reading(r?.memory, bytes);
    case "disk":
      return reading(r?.disk, (d) => bytes(d.free));
    case "network":
      return reading(r?.network, (n) => rate(n.rx));
  }
}

export function caption(id: ResourceId, desktop: boolean): string {
  switch (id) {
    case "cpu":
      return desktop ? "Processor utilization" : "App CPU";
    case "memory":
      return desktop ? "Physical memory in use" : "Resident memory";
    case "disk":
      return "Available storage";
    case "network":
      return "Download";
  }
}

export function figures(id: ResourceId, r: Reading | null, h: History, desktop: boolean): Figure[] {
  const apps = r?.apps ?? [];
  const busiest = apps.reduce<(typeof apps)[number] | null>((top, a) => (!top || a.cpu > top.cpu ? a : top), null);
  const largest = apps.reduce<(typeof apps)[number] | null>((top, a) => (!top || a.memory > top.memory ? a : top), null);
  switch (id) {
    case "cpu":
      return [
        { title: "Average", value: reading(average(h.cpu), (v) => `${v.toFixed(1)}%`), note: "Since launch · last 60 samples", mobile: true },
        { title: "Peak", value: reading(peak(h.cpu), (v) => `${v.toFixed(1)}%`), note: "Last 60 samples", mobile: true },
        { title: "Cores", value: r && r.cores > 0 ? String(r.cores) : "Unavailable", note: "Logical processors", mobile: true },
        {
          title: "Top app",
          value: busiest?.name ?? "Unavailable",
          note: busiest ? `${busiest.cpu.toFixed(1)}% CPU` : "Restricted or no data",
          mobile: false,
        },
      ];
    case "memory":
      return [
        { title: "Physical memory", value: r && r.total > 0 ? bytes(r.total) : "Unavailable", note: "Device capacity", mobile: true },
        { title: "Used", value: reading(r?.memory, bytes), note: desktop ? "System memory" : "Resident memory", mobile: true },
        {
          title: "Available",
          value: desktop ? reading(r?.memory, (m) => bytes(Math.max(0, r!.total - m))) : "Restricted",
          note: "System memory",
          mobile: false,
        },
        { title: "Top app", value: largest?.name ?? "Unavailable", note: largest ? bytes(largest.memory) : "Restricted or no data", mobile: false },
      ];
    case "disk":
      return [
        { title: "Used", value: reading(r?.disk, (d) => bytes(Math.max(0, d.total - d.free))), note: "Selected volume", mobile: true },
        { title: "Free", value: reading(r?.disk, (d) => bytes(d.free)), note: "Available storage", mobile: true },
        { title: "Capacity", value: reading(r?.disk, (d) => bytes(d.total)), note: "Selected volume", mobile: true },
        { title: "Volumes", value: reading(r?.disk, () => String(r!.volumes.length)), note: "Visible mounted volumes", mobile: true },
      ];
    case "network":
      return [
        { title: "Downloading", value: reading(r?.network, (n) => rate(n.rx)), note: "All visible interfaces", mobile: true },
        { title: "Uploading", value: reading(r?.network, (n) => rate(n.tx)), note: "All visible interfaces", mobile: true },
        { title: "Interfaces", value: reading(r?.network, () => String(r!.interfaces.length)), note: "Loopback excluded", mobile: true },
        { title: "Peak download", value: reading(peak(h.download), rate), note: "Last 60 samples · graph scales to fit", mobile: true },
      ];
  }
}

/** The history chart of a resource, with its headline reading. */
export function ResourceChart(props: { id: ResourceId }) {
  const monitor = useMonitor();
  const desktop = monitor.device.scope === "desktop";
  const history = () => monitor.history();
  return (
    <Enter delay={20} class="shrink-0">
      <View
        testID={`${props.id}-history`}
        class={`${PANEL} shrink-0 gap-2 p-4`}
        style={{ ...panelStyle(), ...(desktop ? { height: Math.min(220, Math.max(146, windowSize().height * 0.26)) } : {}) }}
      >
        <View class="flex-row items-center justify-between">
          <Title tint={props.id}>{caption(props.id, desktop)}</Title>
          <Text class={`text-[10px] ${TEXT[props.id]}`}>{monitor.paused() ? "PAUSED" : "LIVE HISTORY"}</Text>
        </View>
        <View class={`gap-4 ${desktop ? "min-h-0 flex-1 flex-row items-center" : ""}`}>
          <View class="w-40 gap-2">
            <Metric value={headline(props.id, monitor.latest())} testID={`${props.id}-reading`} />
            <Label>Latest 60 samples</Label>
            <Label>Sampled every 2 seconds</Label>
          </View>
          <View class={`min-w-[220px] flex-1 ${desktop ? "h-full min-h-0" : ""}`}>
            <LiveChart
              fit={desktop}
              tint={props.id}
              units={props.id === "network" ? "rate" : props.id === "disk" ? "storage" : "percent"}
              memory={props.id === "memory"}
              points={props.id === "network" ? history().download : history()[props.id]}
              secondary={props.id === "network" ? history().upload : undefined}
              secondaryTint="upload"
              paused={monitor.paused()}
            />
          </View>
        </View>
      </View>
    </Enter>
  );
}

function Figures(props: { id: ResourceId }) {
  const monitor = useMonitor();
  const desktop = monitor.device.scope === "desktop";
  const items = () => figures(props.id, monitor.latest(), monitor.history(), desktop).filter((f) => desktop || f.mobile);
  return (
    <View class="shrink-0 flex-row flex-wrap gap-2">
      <Each of={items()} key={(f) => f.title}>
        {(f) => (
          <Stat
            title={f().title}
            value={f().value}
            note={f().note}
            tint={props.id}
            compact={!desktop || windowSize().width < 900}
            testID={`figure-${f().title}`}
          />
        )}
      </Each>
    </View>
  );
}

/** Volumes or interfaces, with the selected volume's read and write rates. */
function Breakdown(props: { id: "disk" | "network" }) {
  const monitor = useMonitor();
  const desktop = monitor.device.scope === "desktop";
  const r = () => monitor.latest();
  const rows = () => (
    <>
      <If when={props.id === "disk"}>
        <Each of={r()?.volumes ?? []} key={(v) => v.name}>
          {(v) => <Row label={v().name} value={`${bytes(v().free)} free / ${bytes(v().total)}`} />}
        </Each>
      </If>
      <If when={props.id === "network"}>
        <Each of={r()?.interfaces ?? []} key={(i) => i.name}>
          {(i) => <Row label={i().name} value={`↓ ${rate(i().rx)}    ↑ ${rate(i().tx)}`} />}
        </Each>
        <If when={(r()?.interfaces.length ?? 0) === 0}>
          <Label>{r() ? "Rates appear after the second sample." : "Reading interfaces…"}</Label>
        </If>
      </If>
    </>
  );
  return (
    <View testID={`${props.id}-breakdown`} class={`${PANEL} gap-2 p-2.5 ${desktop ? "min-h-0 flex-1" : ""}`} style={panelStyle()}>
      <If when={props.id === "disk"}>
        <View class="shrink-0 flex-row flex-wrap gap-3">
          <Text class="text-xs tabular-nums text-foreground">Read {reading(r()?.diskIo, (io) => rate(io.read))}</Text>
          <Text class="text-xs tabular-nums text-foreground">Write {reading(r()?.diskIo, (io) => rate(io.write))}</Text>
        </View>
      </If>
      <View class="shrink-0">
        <Title tint={props.id}>{props.id === "disk" ? "Volumes" : "Network interfaces"}</Title>
      </View>
      <If when={desktop} else={<View class="gap-2">{rows()}</View>}>
        <Scroll class="min-h-0 flex-1" contentClass="gap-2">
          {rows()}
        </Scroll>
      </If>
      <If when={props.id === "network" && r()}>
        <Label>{plural(r()!.interfaces.length, "interface")} · loopback excluded</Label>
      </If>
    </View>
  );
}

export function ResourcePage(props: { id: ResourceId }) {
  const monitor = useMonitor();
  const desktop = monitor.device.scope === "desktop";
  return (
    <View testID={`page-${props.id}`} class="min-h-0 flex-1 gap-2">
      <ResourceChart id={props.id} />
      <Figures id={props.id} />
      <If when={desktop && (props.id === "cpu" || props.id === "memory")}>
        <AppTable memory={props.id === "memory"} />
      </If>
      <If when={props.id === "disk" || props.id === "network"}>
        <Breakdown id={props.id as "disk" | "network"} />
      </If>
    </View>
  );
}
