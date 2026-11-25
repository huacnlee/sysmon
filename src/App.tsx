// The app: one window on the desktops, the tab shell on phones, both reading
// one monitor of the device.
import { platform } from "nsh";
import { Window, isAvailable as hasWindows } from "nsh/desktop";
import { DesktopShell, TITLE_BAR_HEIGHT } from "./Desktop";
import { MobileShell } from "./Mobile";
import type { Source } from "./model";
import { createMonitor, MonitorContext } from "./monitor";

export function App(props: { source: Source }) {
  const monitor = createMonitor(props.source);
  if (!hasWindows || !platform.desktop) {
    return (
      <MonitorContext.Provider value={monitor}>
        <MobileShell />
      </MonitorContext.Provider>
    );
  }
  return (
    <Window
      id="main"
      title="sysmon"
      titleBar={{ style: "hidden", height: TITLE_BAR_HEIGHT }}
      defaultSize={{ width: 1120, height: 760 }}
      minSize={{ width: 520, height: 480 }}
      restoreBounds
    >
      <MonitorContext.Provider value={monitor}>
        <DesktopShell />
      </MonitorContext.Provider>
    </Window>
  );
}
