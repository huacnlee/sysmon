// sysmon: a live view of this device. The UI is TSX; readings come from the
// `sysmon-native` extension (ext/sysmon-native).
import { mount } from "nsh";
import { App } from "./src/App";
import { nativeSource } from "./src/native";

mount(() => <App source={nativeSource} />);
