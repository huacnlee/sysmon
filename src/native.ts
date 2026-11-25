// The app's Source on `sysmon-native`: the extension answers in JSON.
import { Sysmon } from "sysmon-native";
import type { Device, Extras, Reading, Source } from "./model";

export const nativeSource: Source = {
  sample: async () => JSON.parse(await Sysmon.sample()) as Reading,
  extras: async () => JSON.parse(await Sysmon.extras()) as Extras,
  device: () => JSON.parse(Sysmon.device()) as Device,
  uptime: () => Sysmon.uptime(),
};
