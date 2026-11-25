// Applications by resource use: a compact top four for the overview, and the
// searchable, sortable table on the CPU and Memory pages. Search, order and
// the expanded row belong to the table; readings update rows in place.
import { computed, List, signal, Text, View, type ListRef } from "nsh";
import { Button, Input } from "nsh/base";
import { Icon } from "nsh-icon";
import { ArrowDown, ArrowUp, ChevronDown, ChevronRight, Cpu, LayoutGrid, MemoryStick, Search, X } from "nsh-icon-lucide";
import { avatarToken, bytes, initials, orderApps, type AppOrder, type AppUsage } from "./model";
import { useMonitor } from "./monitor";
import { AVATAR, TEXT, type Tint } from "./sections";
import { Hint, Label, Meter, PANEL, panelStyle } from "./ui";

function Avatar(props: { name: string; size: 24 | 26 }) {
  return (
    <View
      class={`shrink-0 items-center justify-center rounded-[7px] ${AVATAR[avatarToken(props.name)]} ${props.size === 24 ? "size-6" : "size-[26px]"}`}
    >
      <Text class={`text-[10px] font-semibold ${TEXT[avatarToken(props.name)]}`}>{initials(props.name)}</Text>
    </View>
  );
}

function Heading(props: { memory: boolean }) {
  const tint: Tint = props.memory ? "memory" : "cpu";
  return (
    <View class="flex-row items-center gap-2">
      <Icon icon={LayoutGrid} size={14} class={TEXT[tint]} />
      <Text role="header" class={`text-[13px] font-semibold ${TEXT[tint]}`}>
        {props.memory ? "Memory by app" : "CPU by app"}
      </Text>
    </View>
  );
}

function share(app: AppUsage, top: number, memory: boolean): number {
  return (memory ? app.memory : app.cpu) / Math.max(1, top);
}

/** The four busiest applications. Rank slots stay mounted; values move. */
export function TopApps(props: { memory: boolean }) {
  const monitor = useMonitor();
  const tint: Tint = props.memory ? "memory" : "cpu";
  const rows = computed(() => orderApps(monitor.latest()?.apps ?? [], "", props.memory ? "memory" : "cpu", true).slice(0, 4));
  const top = computed(() => Math.max(1, ...rows().map((a) => (props.memory ? a.memory : a.cpu))));
  return (
    <View testID="top-apps" class={`${PANEL} min-h-0 w-full min-w-0 flex-1 gap-2.5 p-3`} style={panelStyle()}>
      <View class="shrink-0">
        <Heading memory={props.memory} />
      </View>
      <If when={rows().length === 0}>
        <Label>{monitor.device.scope === "ios" ? "Other applications are restricted by iOS." : "No application metrics available."}</Label>
      </If>
      <View class="min-h-0 flex-1 justify-between gap-1.5">
        {[0, 1, 2, 3].map((index) => (
          <If when={index < rows().length}>
            <View class="min-h-6 shrink-0 flex-row items-center gap-2.5">
              <Avatar name={rows()[index]?.name ?? ""} size={24} />
              <View class="min-w-0 flex-1 gap-[5px]">
                <View class="flex-row justify-between gap-[5px]">
                  <Text class="min-w-0 flex-1 truncate text-xs text-foreground">{rows()[index]?.name ?? ""}</Text>
                  <Text class="text-xs tabular-nums text-muted-foreground">
                    {rows()[index] ? (props.memory ? bytes(rows()[index]!.memory) : `${rows()[index]!.cpu.toFixed(1)}%`) : ""}
                  </Text>
                </View>
                <Meter fraction={rows()[index] ? share(rows()[index]!, top(), props.memory) : 0} tint={tint} />
              </View>
            </View>
          </If>
        ))}
      </View>
    </View>
  );
}

const ORDERS: { order: AppOrder; name: string; id: string; icon: string | null; tint: string }[] = [
  { order: "cpu", name: "CPU", id: "sort-cpu", icon: Cpu, tint: "text-cpu" },
  { order: "memory", name: "Memory", id: "sort-memory", icon: MemoryStick, tint: "text-memory" },
  { order: "name", name: "Name", id: "sort-name", icon: null, tint: "text-name-sort" },
];

/** An inset field; its clear button sits in a fixed trailing slot, so
 * typing moves neither the field nor the sort control. */
function SearchField(props: { query: string; onChange: (value: string) => void }) {
  return (
    <View class="h-8 min-w-[220px] flex-1 flex-row items-center gap-2 rounded-[9px] border border-field-edge bg-field px-2.5 transition-colors duration-150 focus-within:border-field-focus motion-reduce:transition-none">
      <Icon icon={Search} size={14} class="text-muted-foreground" />
      <Input
        testID="apps-search"
        accessibilityLabel="Search applications"
        placeholder="Search applications…"
        value={props.query}
        onChange={({ value }) => props.onChange(value)}
        class="h-[30px] min-w-0 flex-1 border-0 bg-transparent p-0 text-xs text-field-text placeholder:text-field-placeholder"
      />
      <View class="h-6 w-[22px] shrink-0">
        <If when={props.query}>
          <Button
            testID="apps-clear-search"
            accessibilityLabel="Clear application search"
            tooltip={<Hint text="Clear search" />}
            onPress={() => props.onChange("")}
            class="h-6 w-[22px] items-center justify-center rounded-[5px] hover:bg-foreground/7 focus-visible:outline focus-visible:outline-1 focus-visible:outline-ring"
          >
            <Icon icon={X} size={14} class="text-clear-glyph" />
          </Button>
        </If>
      </View>
    </View>
  );
}

function SortControl(props: { order: AppOrder; descending: boolean; onSort: (order: AppOrder) => void }) {
  return (
    <View class="h-8 flex-row gap-0.5 rounded-[9px] border border-segment-edge bg-segment p-0.5">
      {ORDERS.map((item) => {
        const selected = () => props.order === item.order;
        return (
          <Button
            testID={item.id}
            accessibilityLabel={`Sort by ${item.name}${selected() ? (props.descending ? ", descending" : ", ascending") : ""}`}
            selected={selected()}
            tooltip={<Hint text={`Sort by ${item.name}; select again to reverse`} />}
            onPress={() => props.onSort(item.order)}
            class={`h-[26px] flex-row items-center justify-center gap-[5px] rounded-md border px-[9px] transition-colors duration-150 hover:bg-foreground/5 focus-visible:outline focus-visible:outline-1 focus-visible:outline-ring motion-reduce:transition-none ${
              selected() ? "border-segment-selected-edge bg-segment-selected shadow-sm" : "border-transparent"
            }`}
          >
            {item.icon ? (
              <Icon icon={item.icon} size={12} class={selected() ? item.tint : "text-muted-foreground"} />
            ) : (
              <Text class={`text-[10px] font-semibold ${selected() ? item.tint : "text-muted-foreground"}`}>A</Text>
            )}
            <Text class={`text-[11px] font-medium ${selected() ? "text-segment-text" : "text-muted-foreground"}`}>{item.name}</Text>
            <View class="w-2.5 items-center">
              <If when={selected()}>
                <Icon icon={props.descending ? ArrowDown : ArrowUp} size={10} class={item.tint} />
              </If>
            </View>
          </Button>
        );
      })}
    </View>
  );
}

function Detail(props: { label: string; value: string }) {
  return (
    <View class="gap-1">
      <Label>{props.label}</Label>
      <Text class="text-[13px] font-semibold tabular-nums text-foreground">{props.value}</Text>
    </View>
  );
}

/** Every application, searchable and sortable; rows expand for details. */
export function AppTable(props: { memory: boolean }) {
  const monitor = useMonitor();
  const tint: Tint = props.memory ? "memory" : "cpu";
  const query = signal("");
  const order = signal<AppOrder>(props.memory ? "memory" : "cpu");
  const descending = signal(true);
  // By app id: List cells are recycled for other apps.
  const expanded = signal<string | null>(null);
  let list: ListRef | undefined;

  const all = computed(() => monitor.latest()?.apps ?? []);
  const rows = computed(() => orderApps(all(), query(), order(), descending()));
  const top = computed(() => Math.max(1, ...rows().map((a) => (props.memory ? a.memory : a.cpu))));
  const toTop = () => list?.scrollToOffset({ offset: 0, animated: false });

  return (
    <View testID="app-table" class={`${PANEL} min-h-0 w-full min-w-0 flex-1 gap-2.5 p-3`} style={panelStyle()}>
      <View class="shrink-0 flex-row flex-wrap items-center justify-between gap-3">
        <Heading memory={props.memory} />
        <Label>
          {rows().length === all().length ? `${rows().length} applications` : `${rows().length} of ${all().length} applications`}
        </Label>
      </View>
      <View class="shrink-0 flex-row flex-wrap items-center gap-3">
        <SearchField
          query={query()}
          onChange={(value) => {
            query.set(value);
            toTop();
          }}
        />
        <SortControl
          order={order()}
          descending={descending()}
          onSort={(by) => {
            if (order() === by) descending.update((d) => !d);
            else {
              order.set(by);
              descending.set(by !== "name");
            }
            toTop();
          }}
        />
      </View>
      <View class="shrink-0 flex-row justify-between px-2">
        <Label>APPLICATION</Label>
        <Label>{props.memory ? "MEMORY / CPU" : "CPU / MEMORY"}</Label>
      </View>
      <List
        ref={list}
        testID="apps-list"
        accessibilityLabel="Application resources"
        each={rows()}
        keyExtractor={(app) => app.id}
        estimatedItemSize={52}
        contentClass="gap-0.5"
        class="min-h-0 w-full flex-1"
        empty={
          <View class="gap-2 py-6">
            <Text class="text-xs text-foreground">{query() ? "No matching applications" : "Application metrics unavailable"}</Text>
            <Label>
              {query()
                ? "Try a shorter name or clear the search."
                : monitor.device.scope === "ios"
                  ? "iOS does not permit inspection of other applications."
                  : "No application resource readings are available yet."}
            </Label>
          </View>
        }
      >
        {(app, index) => {
          const open = () => expanded() === app().id;
          return (
            <View class={`overflow-hidden rounded-(--radius) ${open() ? "bg-row-open" : "bg-row"}`}>
              <View
                testID="app-row"
                role="button"
                accessibilityLabel={`${open() ? "Close" : "Open"} details for ${app().name}`}
                onPress={() => {
                  if (open()) expanded.set(null);
                  else {
                    expanded.set(app().id);
                    list?.scrollToIndex({ index: index(), animated: true });
                  }
                }}
                class="min-h-[52px] flex-row items-center gap-2.5 px-2.5 py-2 transition-all duration-150 hover:bg-row-hover active:scale-[0.985] motion-reduce:transition-none"
              >
                <Avatar name={app().name} size={26} />
                <View class="min-w-0 flex-1 gap-1">
                  <Text class="truncate text-xs font-medium text-foreground">{app().name}</Text>
                  <Label>{`${app().processes} ${app().processes === 1 ? "process" : "processes"}`}</Label>
                </View>
                <View class="w-[100px] gap-2">
                  <View class="items-end gap-1">
                    <Text class="text-xs font-semibold tabular-nums text-foreground">
                      {props.memory ? bytes(app().memory) : `${app().cpu.toFixed(1)}%`}
                    </Text>
                    <Label class="tabular-nums">{props.memory ? `${app().cpu.toFixed(1)}% CPU` : bytes(app().memory)}</Label>
                  </View>
                  <Meter fraction={share(app(), top(), props.memory)} tint={tint} />
                </View>
                <Icon icon={open() ? ChevronDown : ChevronRight} size={14} class="text-muted-foreground" />
              </View>
              <If when={open()}>
                <View class="gap-2.5 px-2.5 pb-2.5">
                  <View class="h-px bg-row-divider" />
                  <View class="flex-row flex-wrap gap-3.5">
                    <Detail label="CPU" value={`${app().cpu.toFixed(1)}%`} />
                    <Detail label="Resident memory" value={bytes(app().memory)} />
                    <Detail label="Processes" value={String(app().processes)} />
                  </View>
                </View>
              </If>
            </View>
          );
        }}
      </List>
    </View>
  );
}
