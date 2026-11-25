// The desktop window: an app-drawn title bar that keeps CPU, GPU, memory and
// disk rates in view and navigates between sections, over the current page.
import { keymap, signal, Text, View, windowSize, type JSX } from "nsh";
import { Button, Menu, MenuItem, MenuSeparator, TitleBar } from "nsh/base";
import { titleBar } from "nsh/desktop";
import { Icon } from "nsh-icon";
import { ArrowDown, ArrowUp, Ellipsis } from "nsh-icon-lucide";
import { bytes, rate, type Reading } from "./model";
import { useMonitor } from "./monitor";
import { BG, MORE_SECTIONS, SECTIONS, type SectionId } from "./sections";
import { Enter, Hint, Label, Metric, SectionIcon } from "./ui";
import { BatteryPage, BluetoothPage, GpuPage, NeuralPage, SoundPage } from "./pages/Device";
import { Overview } from "./pages/Overview";
import { ResourcePage } from "./pages/Resource";
import { ContainersPage, WorkloadsPage } from "./pages/Workspace";

/** The title bar's height, which the window's controls center in. */
export const TITLE_BAR_HEIGHT = 40;

const MORE = new Set<SectionId>(MORE_SECTIONS.flatMap((g) => g.ids));

/** The current section's quiet baseline under its destination. */
function Baseline(props: { current: SectionId; shown: boolean }) {
  return (
    <View
      accessible={false}
      class={`absolute inset-x-1.5 bottom-0 h-0.5 ${BG[props.current]}`}
      style={{ opacity: props.shown ? 1 : 0 }}
    />
  );
}

const PILL =
  "relative h-7 shrink-0 flex-row items-center rounded-[4px] bg-chrome px-1.5 transition-colors duration-150 hover:bg-chrome-hover focus-visible:bg-chrome-hover motion-reduce:transition-none";

/** A destination in the title bar with its live reading. */
function BarButton(props: {
  section: SectionId;
  current: SectionId;
  label: string;
  hint: string;
  onOpen: (s: SectionId) => void;
  testID: string;
  children: JSX.Element;
}) {
  return (
    <Button
      testID={props.testID}
      accessibilityLabel={props.label}
      selected={props.current === props.section}
      tooltip={<Hint text={props.hint} />}
      onPress={() => props.onOpen(props.section)}
      class={`${PILL} gap-[5px]`}
    >
      {props.children}
      <Baseline current={props.current} shown={props.current === props.section} />
    </Button>
  );
}

function reads<T>(r: Reading | null, read: (r: Reading) => T | null | undefined, format: (v: T) => string): string {
  const v = r ? read(r) : null;
  return v == null ? "—" : format(v);
}

function AppTitleBar(props: { section: SectionId; onOpen: (s: SectionId) => void }) {
  const monitor = useMonitor();
  const r = () => monitor.latest();
  return (
    <TitleBar
      testID="titlebar"
      class="shrink-0 justify-between gap-2 border-b border-chrome-edge bg-chrome"
      // Clear of the window controls, at least 20 points from each edge.
      style={{ paddingLeft: Math.max(titleBar().left, 12) + 8, paddingRight: Math.max(titleBar().right, 12) + 8 }}
    >
      <View class="min-w-0 flex-1">
        <If when={windowSize().width >= 760}>
          <Text class="text-xs font-semibold text-foreground">sysmon</Text>
        </If>
      </View>
      <View class="shrink-0 flex-row items-center gap-1">
        <Button
          testID="titlebar-overview"
          accessibilityLabel="Overview"
          selected={props.section === "overview"}
          tooltip={<Hint text="Overview" />}
          onPress={() => props.onOpen("overview")}
          class={`${PILL} w-7 justify-center px-0`}
        >
          <SectionIcon section="overview" class={props.section === "overview" ? "text-overview" : "text-muted-foreground"} />
          <Baseline current={props.section} shown={props.section === "overview"} />
        </Button>
        <BarButton
          testID="titlebar-cpu"
          section="cpu"
          current={props.section}
          label={`CPU ${reads(r(), (x) => x.cpu, (v) => `${v.toFixed(0)}%`)}. Open CPU details`}
          hint="CPU"
          onOpen={props.onOpen}
        >
          <SectionIcon section="cpu" size={15} />
          <View class="w-[35px]">
            <Metric size="small" value={reads(r(), (x) => x.cpu, (v) => `${v.toFixed(0)} %`)} />
          </View>
        </BarButton>
        <BarButton
          testID="titlebar-gpu"
          section="gpu"
          current={props.section}
          label={`GPU ${reads(r(), (x) => x.gpu, (v) => `${v.toFixed(0)}%`)}. Open GPU details`}
          hint="GPU"
          onOpen={props.onOpen}
        >
          <SectionIcon section="gpu" size={15} />
          <View class="w-[35px]">
            <Metric size="small" value={reads(r(), (x) => x.gpu, (v) => `${v.toFixed(0)} %`)} />
          </View>
        </BarButton>
        <BarButton
          testID="titlebar-memory"
          section="memory"
          current={props.section}
          label={`Memory ${reads(r(), (x) => x.memory, bytes)}. Open memory details`}
          hint="Memory"
          onOpen={props.onOpen}
        >
          <SectionIcon section="memory" size={15} />
          <View class="w-[62px]">
            <Metric size="small" value={reads(r(), (x) => x.memory, bytes)} />
          </View>
        </BarButton>
        <BarButton
          testID="titlebar-disk"
          section="disk"
          current={props.section}
          label={`Disk read ${reads(r(), (x) => x.diskIo, (io) => rate(io.read))}, write ${reads(r(), (x) => x.diskIo, (io) => rate(io.write))}. Open storage details`}
          hint="Disk read / write"
          onOpen={props.onOpen}
        >
          <SectionIcon section="disk" size={15} />
          <View class="flex-row items-center gap-1">
            <View class="w-[72px] flex-row items-center gap-[3px]">
              <Icon icon={ArrowDown} size={10} class="text-muted-foreground" />
              <Metric size="small" value={reads(r(), (x) => x.diskIo, (io) => rate(io.read))} />
            </View>
            <View class="w-[72px] flex-row items-center gap-[3px]">
              <Icon icon={ArrowUp} size={10} class="text-muted-foreground" />
              <Metric size="small" value={reads(r(), (x) => x.diskIo, (io) => rate(io.write))} />
            </View>
          </View>
        </BarButton>
        <Menu
          placement="bottom"
          align="end"
          offset={6}
          margin={8}
          popupClass="w-[224px] gap-1 rounded-[10px] border border-menu-edge bg-menu p-1"
          trigger={
            <Button
              testID="titlebar-more"
              accessibilityLabel="More categories"
              selected={MORE.has(props.section)}
              tooltip={<Hint text="More" />}
              class={`${PILL} w-7 justify-center px-0`}
            >
              <Icon icon={Ellipsis} size={16} class="text-clear-glyph" />
              <Baseline current={props.section} shown={MORE.has(props.section)} />
            </Button>
          }
        >
          {MORE_SECTIONS.map((group, i) => (
            <>
              {i > 0 ? <MenuSeparator class="mx-2 my-1 h-px bg-menu-separator" /> : null}
              <View class="px-3 pb-1 pt-2">
                <Text class="text-[10px] text-muted-foreground">{group.heading}</Text>
              </View>
              {group.ids.map((id) => (
                <MenuItem
                  testID={`menu-${id}`}
                  accessibilityLabel={SECTIONS[id].title}
                  checked={props.section === id}
                  role="menuitemradio"
                  onSelect={() => props.onOpen(id)}
                  class={`flex-row items-center gap-3 rounded-[6px] px-3 py-2 hover:bg-menu-hover focus-visible:bg-menu-hover ${props.section === id ? "bg-menu-selected" : "bg-menu"}`}
                >
                  <SectionIcon section={id} />
                  <Text class="text-xs text-foreground">{SECTIONS[id].title}</Text>
                </MenuItem>
              ))}
            </>
          ))}
        </Menu>
      </View>
    </TitleBar>
  );
}

function Page(props: { section: SectionId; onOpen: (s: SectionId) => void }): JSX.Element {
  switch (props.section) {
    case "overview":
      return <Overview onOpen={props.onOpen} />;
    case "cpu":
    case "memory":
    case "disk":
    case "network":
      return <ResourcePage id={props.section} />;
    case "gpu":
      return <GpuPage />;
    case "neural":
      return <NeuralPage />;
    case "battery":
      return <BatteryPage />;
    case "sound":
      return <SoundPage />;
    case "bluetooth":
      return <BluetoothPage />;
    case "projects":
    case "agents":
      return <WorkloadsPage kind={props.section} />;
    case "containers":
      return <ContainersPage />;
  }
}

const SHORTCUTS: SectionId[] = ["overview", "cpu", "memory", "disk", "network", "gpu"];

export function DesktopShell() {
  const monitor = useMonitor();
  const section = signal<SectionId>("overview");
  SHORTCUTS.forEach((id, i) => keymap.bind(`mod+${i + 1}`, () => section.set(id)));
  // Charts mark a pause with "paused" in place of "latest".
  keymap.bind("space", () => monitor.setPaused(!monitor.paused()));
  return (
    <View class="flex-1 bg-background">
      <AppTitleBar section={section()} onOpen={(s) => section.set(s)} />
      <View class={`min-h-0 min-w-0 flex-1 ${windowSize().width < 600 ? "p-2.5" : "p-4"}`}>
        <View testID="desktop-content" class="min-h-0 w-full flex-1 overflow-hidden">
          <If
            when={monitor.latest()}
            else={
              <View class="p-5">
                <Label>{monitor.failure() ?? "Reading your device…"}</Label>
              </View>
            }
          >
            {/* A new page for each section; readings update it in place. */}
            {(() => {
              const id = section();
              return (
                <Enter class="min-h-0 w-full flex-1">
                  <Page section={id} onOpen={(s) => section.set(s)} />
                </Enter>
              );
            })()}
          </If>
        </View>
      </View>
    </View>
  );
}
