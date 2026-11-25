// Development projects and coding assistants found among local processes, and
// Docker containers grouped by Compose project.
import { computed, Scroll, signal, Text, View, windowSize } from "nsh";
import { Icon } from "nsh-icon";
import { ChevronDown, ChevronRight } from "nsh-icon-lucide";
import { bytes, containerTone, duration, groupContainers, portsOf, reports, type Container, type Workload } from "../model";
import { useMonitor } from "../monitor";
import { Chip, IconTile, Label, PANEL, panelStyle, Progress, Row, SectionIcon, Stat, Title } from "../ui";

export function WorkloadsPage(props: { kind: "projects" | "agents" }) {
  const monitor = useMonitor();
  // The open row, by its group's name and process: a reading keeps it open.
  const expanded = signal<string | null>(null);
  const items = computed(() => monitor.latest()?.workloads[props.kind] ?? []);
  const projects = props.kind === "projects";
  const compact = () => windowSize().width < 900;
  return (
    <View testID={`page-${props.kind}`} class="min-h-0 flex-1 gap-2">
      <View class="shrink-0 flex-row flex-wrap gap-2">
        <Stat
          compact={compact()}
          tint={props.kind}
          title={projects ? "Development projects" : "Your coding assistants"}
          value={String(items().length)}
          note="Discovered on this device"
        />
        <Stat compact={compact()} tint={props.kind} title="Memory" value={bytes(items().reduce((s, i) => s + i.memory, 0))} note="Combined resident memory" />
        <Stat compact={compact()} tint={props.kind} title="CPU" value={`${items().reduce((s, i) => s + i.cpu, 0).toFixed(1)}%`} note="100% equals one core" />
        <Stat
          compact={compact()}
          tint={props.kind}
          title="Processes"
          value={String(items().reduce((s, i) => s + i.runningProcesses, 0))}
          note="Observed processes"
        />
      </View>
      <View class={`${PANEL} min-h-0 flex-1 gap-2 p-2.5`} style={panelStyle()}>
        <View class="shrink-0 flex-row justify-between">
          <Title tint={props.kind}>{projects ? "Projects" : "Local sessions"}</Title>
          <Label>CPU / MEMORY</Label>
        </View>
        <Scroll testID="workload-list" class="min-h-0 flex-1" contentClass="gap-2">
          <If when={items().length === 0}>
            <View class="py-6">
              <Label>
                {monitor.device.scope !== "desktop"
                  ? "This platform restricts discovery of other applications."
                  : projects
                    ? "No development processes detected. Start a development server to see it here."
                    : "No coding assistant processes detected."}
              </Label>
            </View>
          </If>
          <Each of={items()} key={(w) => `${w.name}:${w.pid}`}>
            {(item) => (
              <WorkloadRow
                item={item()}
                kind={props.kind}
                ports={portsOf(item(), monitor.extras().listeningPorts)}
                open={expanded() === `${item().name}:${item().pid}`}
                onToggle={() => {
                  const key = `${item().name}:${item().pid}`;
                  expanded.set(expanded() === key ? null : key);
                }}
              />
            )}
          </Each>
        </Scroll>
      </View>
    </View>
  );
}

function WorkloadRow(props: { item: Workload; kind: "projects" | "agents"; ports: string[]; open: boolean; onToggle: () => void }) {
  const idle = () => props.item.cpu < 0.1;
  return (
    <View class={`gap-2 rounded-(--radius) p-3 ${props.open ? "bg-workload-open" : "bg-muted"}`}>
      <View
        testID="workload-row"
        role="button"
        accessibilityLabel={`${props.open ? "Close" : "Open"} details for ${props.item.name}`}
        onPress={props.onToggle}
        class="flex-row flex-wrap items-center justify-between gap-2"
      >
        <View class="min-w-0 flex-row items-center gap-2">
          <IconTile section={props.kind} />
          <View class="min-w-0 gap-1">
            <Title>{props.item.name}</Title>
            <Label>{props.item.detail}</Label>
          </View>
        </View>
        <View class="flex-row flex-wrap items-center gap-2">
          <Each of={props.ports.slice(0, 5)}>{(port) => <Chip tint={props.kind}>{port()}</Chip>}</Each>
          <Chip status={idle() ? "muted" : "active"}>{idle() ? "Low CPU" : "Active"}</Chip>
          <Label class="tabular-nums">{`${props.item.cpu.toFixed(1)}%`}</Label>
          <Text class="text-base font-semibold tabular-nums text-foreground">{bytes(props.item.memory)}</Text>
          <Icon icon={props.open ? ChevronDown : ChevronRight} size={14} class="text-muted-foreground" />
        </View>
      </View>
      <If when={props.open}>
        <View class="gap-2 pl-4">
          <Row label="Longest-running process" value={duration(props.item.startedSeconds)} />
          <Row label="Observed processes" value={String(props.item.runningProcesses)} />
          <View class="flex-row flex-wrap gap-2">
            <Each of={props.item.pids}>{(pid) => <Chip>{`PID ${pid()}`}</Chip>}</Each>
          </View>
        </View>
      </If>
    </View>
  );
}

export function ContainersPage() {
  const monitor = useMonitor();
  const all = () => monitor.extras().containers;
  const memoryReporting = () => all().filter((c) => reports(c.memory)).length;
  const cpuReporting = () => all().filter((c) => reports(c.cpu)).length;
  const compact = () => windowSize().width < 900;
  return (
    <View testID="page-containers" class="min-h-0 flex-1 gap-2">
      <View class="shrink-0 flex-row flex-wrap gap-2">
        <Stat compact={compact()} tint="containers" title="Containers" value={String(all().length)} note="Discovered through Docker" />
        <Stat
          compact={compact()}
          tint="containers"
          title="Memory"
          value={memoryReporting() > 0 ? bytes(all().reduce((s, c) => s + c.memoryUsed, 0)) : "Unavailable"}
          note={`${memoryReporting()} of ${all().length} reporting`}
        />
        <Stat
          compact={compact()}
          tint="containers"
          title="CPU"
          value={cpuReporting() > 0 ? `${all().reduce((s, c) => s + c.cpuPercent, 0).toFixed(1)}%` : "Unavailable"}
          note="Combined container activity"
        />
        <Stat
          compact={compact()}
          tint="containers"
          title="With published ports"
          value={String(all().filter((c) => c.ports).length)}
          note="Containers exposing ports"
        />
      </View>
      <View class="shrink-0 gap-2 rounded-(--radius) border border-border p-2.5">
        <Title tint="containers">Containers</Title>
        <Label>{monitor.extras().containerStatus || "Discovering Docker engine…"}</Label>
      </View>
      <Scroll testID="containers-list" class="min-h-0 flex-1" contentClass="gap-2">
        <If when={all().length === 0}>
          <Label>No container rows are available. Start Docker and check access to its configured engine.</Label>
        </If>
        <Each of={groupContainers(all())} key={(g) => g.name}>
          {(group) => (
            <View class={`${PANEL} gap-3 p-3`} style={panelStyle()}>
              <View class="flex-row items-center justify-between gap-2">
                <View class="min-w-0 flex-1 gap-1">
                  <Title>{group().name}</Title>
                  <Label>{`${group().containers.length} containers`}</Label>
                </View>
                <Text class="text-lg tabular-nums text-foreground">
                  {group().containers.some((c) => reports(c.memory))
                    ? bytes(group().containers.reduce((s, c) => s + c.memoryUsed, 0))
                    : "Unavailable"}
                </Text>
              </View>
              <Each of={group().containers} key={(c) => c.name}>
                {(c) => <ContainerRow container={c()} />}
              </Each>
            </View>
          )}
        </Each>
      </Scroll>
    </View>
  );
}

function ContainerRow(props: { container: Container }) {
  const c = () => props.container;
  const usage = () => (c().memoryUsed / Math.max(1, c().memoryLimit)) * 100;
  return (
    <View testID="container-row" class="gap-2 rounded-[10px] border border-container-edge p-3">
      <View class="flex-row flex-wrap items-center justify-between gap-2">
        <View class="min-w-40 flex-1 gap-1">
          <View class="flex-row items-center gap-2">
            <SectionIcon section="containers" size={17} />
            <Title>{c().name}</Title>
          </View>
          <Label>{c().image}</Label>
        </View>
        <View class="gap-1">
          <Text class="text-[13px] tabular-nums text-foreground">{c().memory}</Text>
          <Label>{`CPU ${c().cpu}`}</Label>
        </View>
      </View>
      <View class="flex-row flex-wrap justify-between gap-2">
        <Chip status={containerTone(c().status)}>{c().status}</Chip>
        <If when={c().ports}>
          <Chip tint="containers">{c().ports}</Chip>
        </If>
      </View>
      <If when={reports(c().memory)}>
        <View class="flex-row flex-wrap items-center gap-2">
          <Progress percent={usage()} tint="containers" high={usage() > 85} width={120} />
          <Label>{`${usage().toFixed(1)}% of ${bytes(c().memoryLimit)} limit`}</Label>
          <Label>{`Transferred ↓ ${bytes(c().networkRx)} / ↑ ${bytes(c().networkTx)}`}</Label>
        </View>
      </If>
    </View>
  );
}
