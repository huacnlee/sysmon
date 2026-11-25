// Phones: the system tab bar for the four sections, and a navigation stack
// that opens a resource's detail from Overview or Activity. Readings on phones
// describe sysmon itself; other apps are the system's to hide.
import { appInfo, platform, Scroll, signal, Text, View } from "nsh";
import { Button, createNavStack, NavPage, NavStack } from "nsh/base";
import { TabBar } from "nsh/mobile";
import { Icon } from "nsh-icon";
import { ArrowLeft, ChevronRight } from "nsh-icon-lucide";
import { bytes, percent, rate, reading } from "./model";
import { useMonitor } from "./monitor";
import { ResourceCard } from "./pages/Overview";
import { ResourcePage, type ResourceId } from "./pages/Resource";
import { SECTIONS } from "./sections";
import { IconTile, Label, Metric, PANEL, panelStyle, Row, Title } from "./ui";

type Route = { name: "home" } | { name: "resource"; id: ResourceId };

const TABS = ["Overview", "Activity", "Device", "About"] as const;
const ICONS = {
  ios: ["square.grid.2x2", "waveform.path.ecg", "iphone", "info.circle"],
  android: ["android:ic_menu_view", "android:ic_menu_sort_by_size", "android:ic_menu_manage", "android:ic_menu_info_details"],
};

const RESOURCES: ResourceId[] = ["cpu", "memory", "disk", "network"];

function resourceTitle(id: ResourceId): string {
  return id === "disk" ? "Storage" : SECTIONS[id].title;
}

/** The desktop and web header of a page; phones show the system's. */
function Header(props: { title: string; onBack?: () => void }) {
  return (
    <View class="h-12 flex-row items-center gap-2 border-b border-border px-4">
      <If when={props.onBack}>
        <Button
          accessibilityLabel="Back"
          onPress={() => props.onBack!()}
          class="size-8 items-center justify-center rounded-(--radius-control) active:bg-muted"
        >
          <Icon icon={ArrowLeft} size={18} class="text-foreground" />
        </Button>
      </If>
      <Text role="header" class="text-base font-semibold text-foreground">
        {props.title}
      </Text>
    </View>
  );
}

function MobileOverview(props: { onOpen: (id: ResourceId) => void }) {
  const monitor = useMonitor();
  const r = () => monitor.latest();
  const h = () => monitor.history();
  return (
    <View class="gap-2">
      <ResourceCard
        delay={0}
        section="cpu"
        title="App CPU"
        value={reading(r()?.cpu, (v) => percent(v, 1))}
        note={`${r()?.cores ?? 0} logical cores`}
        points={h().cpu}
        units="percent"
        onOpen={() => props.onOpen("cpu")}
      />
      <ResourceCard
        delay={40}
        section="memory"
        title="Resident memory"
        value={reading(r()?.memory, bytes)}
        note="App memory in use"
        points={h().memory}
        units="percent"
        memory
        onOpen={() => props.onOpen("memory")}
      />
      <ResourceCard
        delay={80}
        section="disk"
        title="Storage"
        value={reading(r()?.disk, (d) => bytes(d.free))}
        note="Available on the data volume"
        points={h().disk}
        units="storage"
        onOpen={() => props.onOpen("disk")}
      />
      <ResourceCard
        delay={120}
        section="network"
        title="Network"
        value={reading(r()?.network, (n) => rate(n.rx))}
        note={`↑ ${reading(r()?.network, (n) => rate(n.tx))}`}
        points={h().download}
        secondary={h().upload}
        units="rate"
        onOpen={() => props.onOpen("network")}
      />
      <View class={`${PANEL} gap-2 p-2.5`} style={panelStyle()}>
        <Title tint="battery">Battery</Title>
        <Text class="text-sm text-foreground">
          {monitor.extras().battery
            ? `${monitor.extras().battery!.percent.toFixed(0)}% · ${monitor.extras().battery!.status}`
            : "Not available on this device"}
        </Text>
      </View>
    </View>
  );
}

function Activity(props: { onOpen: (id: ResourceId) => void }) {
  const monitor = useMonitor();
  const r = () => monitor.latest();
  const summary = (id: ResourceId) => {
    const x = r();
    switch (id) {
      case "cpu":
        return reading(x?.cpu, (v) => `${v.toFixed(1)}%`);
      case "memory":
        return reading(x?.memory, (m) => `${bytes(m)} · resident`);
      case "disk":
        return reading(x?.disk, (d) => `${bytes(d.free)} available`);
      case "network":
        return reading(x?.network, (n) => `↓ ${rate(n.rx)}  ↑ ${rate(n.tx)}`);
    }
  };
  return (
    <View class="gap-4">
      <Label>Tap a resource to inspect its readings and history.</Label>
      <View class="overflow-hidden rounded-2xl border border-border bg-card" style={panelStyle()}>
        {RESOURCES.map((id) => (
          <View
            testID={`mobile-resource-${id}`}
            role="button"
            accessibilityLabel={resourceTitle(id)}
            onPress={() => props.onOpen(id)}
            class="min-h-18 flex-row items-center gap-3 px-4 py-3 active:bg-row"
          >
            <IconTile section={id} />
            <View class="flex-1 gap-1">
              <Title>{resourceTitle(id)}</Title>
              <Text class="text-xs text-muted-foreground">{summary(id)}</Text>
            </View>
            <Icon icon={ChevronRight} size={20} class="text-muted-foreground" />
          </View>
        ))}
      </View>
    </View>
  );
}

function DeviceTab() {
  const monitor = useMonitor();
  const e = () => monitor.extras();
  const r = () => monitor.latest();
  return (
    <View class="gap-4">
      <View class={`${PANEL} gap-3 p-4`} style={panelStyle()}>
        <Title>This device</Title>
        <Text class="text-base text-foreground">{e().deviceModel ?? "This device"}</Text>
        <Text class="text-xs text-muted-foreground">{e().osVersion ?? monitor.device.name}</Text>
        <Row label="Architecture" value={monitor.device.arch} />
        <Text class="text-xs text-muted-foreground">
          {`${r()?.cores ?? 0} logical cores · ${r() && r()!.total > 0 ? bytes(r()!.total) : "Unavailable"} physical memory`}
        </Text>
        <Text class="text-xs text-muted-foreground">{`Storage available  ${reading(r()?.disk, (d) => bytes(d.free))}`}</Text>
      </View>
      <View class={`${PANEL} gap-3 p-4`} style={panelStyle()}>
        <Title tint="battery">Battery</Title>
        <Metric value={reading(e().battery, (b) => `${b.percent.toFixed(0)} %`)} />
        <Text class="text-[13px] text-muted-foreground">{e().battery?.status ?? "No accessible battery reading"}</Text>
      </View>
      <If when={e().batteryStatus}>
        <Label>{e().batteryStatus}</Label>
      </If>
      <If when={e().sound}>
        <View class={`${PANEL} gap-2 p-4`} style={panelStyle()}>
          <Title>App audio output</Title>
          <Text class="text-sm text-foreground">{e().sound!}</Text>
        </View>
      </If>
    </View>
  );
}

function About() {
  const monitor = useMonitor();
  return (
    <View class="gap-4">
      <View class={`${PANEL} gap-3 p-4`} style={panelStyle()}>
        <Title tint="overview">sysmon</Title>
        <Row label="Version" value={appInfo().version ?? "Development"} />
        <Row label="Sampling interval" value="2 seconds" />
        <Row label="History" value="Latest 60 samples · kept in memory" />
        <Label>
          Readings are kept on your device and discarded when the app closes. No account, analytics or cloud service is used.
        </Label>
      </View>
      <View class={`${PANEL} gap-3 p-4`} style={panelStyle()}>
        <Title>Sampling</Title>
        <Label>
          Pause holds the current readings and history. Resume continues collecting; historical gaps keep their actual duration.
        </Label>
        <Button
          testID="mobile-about-sampling"
          accessibilityLabel={monitor.paused() ? "Resume readings" : "Pause readings"}
          onPress={() => monitor.setPaused(!monitor.paused())}
          class="min-h-12 items-center justify-center rounded-[10px] bg-mobile-action active:opacity-80"
        >
          <Text class="text-[15px] text-mobile-action-foreground">{monitor.paused() ? "Resume readings" : "Pause readings"}</Text>
        </Button>
      </View>
      <View class="gap-2 rounded-(--radius) bg-scope p-3">
        <Text role="header" class="text-[13px] font-semibold text-scope-foreground">
          Data scope
        </Text>
        <Label>
          CPU and resident memory describe sysmon. Storage and network describe what this app can access. The OS protects other
          apps; their activity is not shown.
        </Label>
      </View>
    </View>
  );
}

export function MobileShell() {
  const monitor = useMonitor();
  const tab = signal(0);
  const nav = createNavStack<Route>([{ name: "home" }]);
  const open = (id: ResourceId) => nav.push({ name: "resource", id });
  const icons = platform.os === "android" ? ICONS.android : ICONS.ios;
  return (
    <View class="flex-1 bg-background px-[env(safe-area-inset-left)]">
      <NavStack state={nav} class="min-h-0 flex-1">
        {(route) =>
          route.name === "home" ? (
            <NavPage title={TABS[tab()]} header={<Header title={TABS[tab()]} />} headerClass="bg-background text-foreground tint-foreground">
              <Scroll class="min-h-0 flex-1" contentClass="px-4 pb-5 pt-2">
                <If
                  when={monitor.latest()}
                  else={
                    <View class="py-6">
                      <Label>{monitor.failure() ?? "Reading your device…"}</Label>
                    </View>
                  }
                >
                  {(() => {
                    switch (tab()) {
                      case 0:
                        return <MobileOverview onOpen={open} />;
                      case 1:
                        return <Activity onOpen={open} />;
                      case 2:
                        return <DeviceTab />;
                      default:
                        return <About />;
                    }
                  })()}
                </If>
              </Scroll>
            </NavPage>
          ) : (
            <NavPage
              title={resourceTitle(route.id)}
              header={<Header title={resourceTitle(route.id)} onBack={() => nav.pop()} />}
              headerClass="bg-background text-foreground tint-foreground"
            >
              <Scroll class="min-h-0 flex-1" contentClass="gap-3 px-4 pb-5 pt-2">
                <ResourcePage id={route.id} />
              </Scroll>
            </NavPage>
          )
        }
      </NavStack>
      <TabBar
        items={TABS.map((title, i) => ({ title, icon: icons[i] }))}
        selectedIndex={tab()}
        onChange={({ index }) => {
          nav.popToRoot();
          tab.set(index);
        }}
      />
    </View>
  );
}
