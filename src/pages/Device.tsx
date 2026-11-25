// Device pages: GPU activity, battery, audio output, Bluetooth, and the Neural
// Engine, which no platform exposes to this app.
import { Scroll, Text, View, windowSize } from "nsh";
import { LiveChart } from "../chart";
import { reading } from "../model";
import { useMonitor } from "../monitor";
import { type SectionId } from "../sections";
import { IconTile, Label, Metric, PANEL, panelStyle, Row, SectionIcon, Stat, Title } from "../ui";

export function GpuPage() {
  const monitor = useMonitor();
  return (
    <View testID="page-gpu" class="min-h-0 flex-1 gap-2">
      <View class={`${PANEL} min-h-0 flex-1 gap-2 p-4`} style={panelStyle()}>
        <View class="flex-row items-center justify-between">
          <Title tint="gpu">GPU activity</Title>
          <Text class="text-[10px] text-gpu">{monitor.paused() ? "PAUSED" : "LIVE HISTORY"}</Text>
        </View>
        <Text class="text-base text-foreground">{monitor.extras().gpu ?? "Graphics processor"}</Text>
        <Metric value={reading(monitor.latest()?.gpu, (v) => `${v.toFixed(1)} %`)} testID="gpu-reading" />
        <Label>Driver-reported activity · busiest visible GPU</Label>
        <LiveChart tint="gpu" units="percent" points={monitor.history().gpu} paused={monitor.paused()} />
        <Label>
          {monitor.latest()?.gpu == null
            ? "The graphics driver does not expose utilization to this app. Hardware identity may still be available."
            : "Read-only system telemetry. Per-app GPU attribution is not collected."}
        </Label>
      </View>
    </View>
  );
}

/** A device fact: a tile, a heading, the value at 20 pt and what it means. */
function Fact(props: { section: SectionId; title: string; value: string; note: string }) {
  return (
    <View class={`${PANEL} min-h-0 flex-1 gap-3 p-4`} style={panelStyle()}>
      <View class="flex-row items-center gap-2">
        <IconTile section={props.section} />
        <Title>{props.title}</Title>
      </View>
      <Text class="text-xl font-medium text-foreground">{props.value}</Text>
      <Label>{props.note}</Label>
    </View>
  );
}

export function BatteryPage() {
  const monitor = useMonitor();
  const battery = () => monitor.extras().battery;
  const compact = () => windowSize().width < 900;
  return (
    <View testID="page-battery" class="min-h-0 flex-1 gap-2">
      <Fact
        section="battery"
        title="Battery"
        value={battery()?.status ?? "No battery information"}
        note={monitor.extras().batteryStatus || "Readings come from the system power service and refresh every 30 seconds."}
      />
      <View class="shrink-0 flex-row flex-wrap gap-2">
        <Stat compact={compact()} tint="battery" title="Charge" value={reading(battery(), (b) => `${b.percent.toFixed(0)}%`)} note="Current battery level" />
        <Stat compact={compact()} tint="battery" title="Remaining" value={battery()?.remaining ?? "Unavailable"} note="System estimate" />
        <Stat
          compact={compact()}
          tint="battery"
          title="Health"
          value={reading(battery()?.health, (v) => `${v.toFixed(0)}%`)}
          note="Full capacity / design capacity"
        />
        <Stat
          compact={compact()}
          tint="battery"
          title="Power draw"
          value={reading(battery()?.powerW, (v) => `${v.toFixed(1)} W`)}
          note="Reported battery power"
        />
      </View>
      <View class="shrink-0 gap-2 rounded-(--radius) border border-border p-3">
        <Row label="Cycle count" value={reading(battery()?.cycles, String)} />
      </View>
    </View>
  );
}

export function SoundPage() {
  const monitor = useMonitor();
  return (
    <View testID="page-sound" class="min-h-0 flex-1 gap-2">
      <Fact
        section="sound"
        title="Audio output"
        value={monitor.extras().sound ?? "No audio information"}
        note="Current system output information. Per-app mixing requires a platform audio-routing provider and is not available here."
      />
    </View>
  );
}

export function BluetoothPage() {
  const monitor = useMonitor();
  return (
    <View testID="page-bluetooth" class={`${PANEL} min-h-0 flex-1 gap-3 p-4`} style={panelStyle()}>
      <View class="shrink-0">
        <Title tint="bluetooth">Bluetooth devices</Title>
      </View>
      <Label class="shrink-0">{monitor.extras().bluetoothStatus || "Discovering available devices…"}</Label>
      <Scroll testID="bluetooth-list" class="min-h-0 flex-1" contentClass="gap-2">
        <Each of={monitor.extras().bluetooth}>
          {(name) => (
            <View class="py-3">
              <Text class="text-base text-foreground">{name()}</Text>
            </View>
          )}
        </Each>
      </Scroll>
    </View>
  );
}

/** A page whose telemetry this device does not provide. */
export function Unavailable(props: { section: SectionId; title: string; body: string; testID?: string }) {
  return (
    <View testID={props.testID} class={`${PANEL} min-h-0 flex-1 justify-center gap-3 p-5`} style={panelStyle()}>
      <View class="flex-row items-center gap-2">
        <SectionIcon section={props.section} size={15} />
        <Title tint={props.section}>{props.title}</Title>
      </View>
      <Label>{props.body}</Label>
    </View>
  );
}

export function NeuralPage() {
  return (
    <Unavailable
      testID="page-neural"
      section="neural"
      title="Neural Engine telemetry unavailable"
      body="This device does not expose Neural Engine activity to this app."
    />
  );
}
