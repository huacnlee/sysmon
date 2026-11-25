// The desktop overview: the three live resources with their histories, a
// device snapshot with the memory-heaviest apps, and the local workspace.
import { Text, View, windowSize } from "nsh";
import { Icon } from "nsh-icon";
import { ArrowUpRight } from "nsh-icon-lucide";
import { TopApps } from "../apps";
import { LiveChart, type ChartUnits } from "../chart";
import { average, bytes, duration, rate, reading, type Point } from "../model";
import { useMonitor } from "../monitor";
import { SECTIONS, type SectionId, type Tint } from "../sections";
import { Enter, Hint, IconTile, INTERACTIVE, Label, Meter, Metric, PANEL, panelStyle, Row, SectionIcon, Title } from "../ui";

/** A resource card: its reading, a note, and its recent history. Opens its page. */
export function ResourceCard(props: {
  section: SectionId;
  title: string;
  value: string;
  note: string;
  points: Point[];
  secondary?: Point[];
  units: ChartUnits;
  memory?: boolean;
  delay: number;
  fit?: boolean;
  onOpen: () => void;
}) {
  const monitor = useMonitor();
  return (
    <Enter delay={props.delay} class={props.fit ? "min-h-0 flex-1" : ""}>
      <View
        testID={`card-${props.section}`}
        role="button"
        accessibilityLabel={`${props.title}: ${props.value}. Open ${SECTIONS[props.section].title}`}
        onPress={props.onOpen}
        class={`${PANEL} ${INTERACTIVE} gap-1.5 p-2 ${props.fit ? "min-h-0 flex-1" : ""}`}
        style={panelStyle()}
      >
        <View class="flex-row items-center justify-between">
          <View class="flex-row items-center gap-2">
            <SectionIcon section={props.section} size={15} />
            <Title tint={props.section}>{props.title}</Title>
          </View>
          <Icon icon={ArrowUpRight} size={14} class="text-muted-foreground" />
        </View>
        <View class={`gap-1.5 ${props.fit ? "min-h-0 flex-1" : ""}`}>
          <View class="shrink-0 flex-row items-end justify-between gap-2">
            <Metric value={props.value} />
            <Text class="min-w-0 flex-1 text-right text-[11px] text-muted-foreground">{props.note}</Text>
          </View>
          <LiveChart
            compact
            fit={props.fit}
            tint={props.section as Tint}
            units={props.units}
            memory={props.memory}
            points={props.points}
            secondary={props.secondary}
            secondaryTint="upload"
            paused={monitor.paused()}
          />
        </View>
      </View>
    </Enter>
  );
}

function Snapshot() {
  const monitor = useMonitor();
  const r = () => monitor.latest();
  const used = () => {
    const x = r();
    return x?.memory != null && x.total > 0 ? x.memory / x.total : 0;
  };
  return (
    <Enter delay={90} testID="device-snapshot" class="shrink-0 gap-2 p-1">
      <Title>Device snapshot</Title>
      <Row label="Memory in use" value={reading(r()?.memory, (m) => `${bytes(m)} of ${bytes(r()!.total)}`)} />
      <Meter fraction={used()} tint="memory" thick track="bg-track" />
      <View class="flex-row flex-wrap gap-2">
        <Title tint="memory">● Used</Title>
        <Label>● Available</Label>
      </View>
      <View class="gap-2 rounded-(--radius) border border-border p-2.5">
        <Row label="Operating system" value={monitor.device.name} />
        <Row label="Host" value={monitor.device.host} />
        <Row label="Storage available" value={reading(r()?.disk, (d) => bytes(d.free))} />
        <Row label="Battery" value={reading(monitor.extras().battery, (b) => `${b.percent.toFixed(0)}% · ${b.status}`)} />
        <Row label="Uptime" value={(r(), reading(monitor.uptime(), duration))} />
      </View>
    </Enter>
  );
}

function WorkspaceTile(props: { section: SectionId; caption: string; count: number; delay: number; onOpen: () => void }) {
  const wide = () => windowSize().width >= 800;
  return (
    <Enter delay={props.delay} class="min-w-0 flex-1">
      <View
        testID={`overview-${props.section}`}
        role="button"
        accessibilityLabel={`${props.caption}: ${props.count}`}
        tooltip={<Hint text={props.caption} />}
        onPress={props.onOpen}
        class={`${INTERACTIVE} min-w-0 flex-row items-center gap-2 rounded-(--radius) border border-border px-2.5 py-2 ${wide() ? "h-14" : "h-11"}`}
      >
        <IconTile section={props.section} />
        <View class="min-w-0 flex-1 gap-1">
          <Text class="text-[15px] tabular-nums text-foreground">{props.count}</Text>
          <If when={wide()}>
            <Label class="truncate">{props.caption}</Label>
          </If>
        </View>
        <Icon icon={ArrowUpRight} size={14} class="text-muted-foreground" />
      </View>
    </Enter>
  );
}

export function Overview(props: { onOpen: (section: SectionId) => void }) {
  const monitor = useMonitor();
  const r = () => monitor.latest();
  const h = () => monitor.history();
  return (
    <View testID="page-overview" class="min-h-0 flex-1 gap-2">
      <View class="min-h-0 flex-1 flex-row gap-2">
        <View class="min-h-0 min-w-0 flex-[1.35] gap-2">
          <ResourceCard
            fit
            delay={35}
            section="cpu"
            title="CPU activity"
            value={reading(r()?.cpu, (v) => `${v.toFixed(0)} %`)}
            note={reading(average(h().cpu), (v) => `${v.toFixed(1)}% average · ${r()?.cores ?? 0} cores`)}
            points={h().cpu}
            units="percent"
            onOpen={() => props.onOpen("cpu")}
          />
          <ResourceCard
            fit
            delay={85}
            section="memory"
            title="Memory usage"
            value={reading(r()?.memory, bytes)}
            note={`${reading(r()?.memory, (m) => bytes(Math.max(0, r()!.total - m)))} available`}
            points={h().memory}
            units="percent"
            memory
            onOpen={() => props.onOpen("memory")}
          />
          <ResourceCard
            fit
            delay={135}
            section="network"
            title="Network transfer"
            value={reading(r()?.network, (n) => rate(n.rx))}
            note={`↑ ${reading(r()?.network, (n) => rate(n.tx))}`}
            points={h().download}
            secondary={h().upload}
            units="rate"
            onOpen={() => props.onOpen("network")}
          />
        </View>
        <If when={windowSize().width >= 700}>
          <View class="min-h-0 min-w-0 flex-1 gap-2">
            <Snapshot />
            <TopApps memory />
          </View>
        </If>
      </View>
      <View class="shrink-0 flex-row gap-2">
        <WorkspaceTile
          delay={180}
          section="projects"
          caption="Development projects"
          count={r()?.workloads.projects.length ?? 0}
          onOpen={() => props.onOpen("projects")}
        />
        <WorkspaceTile
          delay={225}
          section="agents"
          caption="Coding sessions"
          count={r()?.workloads.agents.length ?? 0}
          onOpen={() => props.onOpen("agents")}
        />
        <WorkspaceTile
          delay={270}
          section="containers"
          caption="Docker containers"
          count={monitor.extras().containers.length}
          onOpen={() => props.onOpen("containers")}
        />
      </View>
    </View>
  );
}
