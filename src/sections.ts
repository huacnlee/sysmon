// The destinations of the app, in navigation order, with their icon and the
// theme token that tints them (`theme.json`).
import {
  Battery,
  Bluetooth,
  Bot,
  Brain,
  Cpu,
  Folder,
  Globe,
  Gpu,
  HardDrive,
  LayoutGrid,
  MemoryStick,
  Package,
  Volume2,
} from "nsh-icon-lucide";

export type SectionId =
  | "overview"
  | "cpu"
  | "memory"
  | "disk"
  | "network"
  | "gpu"
  | "neural"
  | "battery"
  | "sound"
  | "bluetooth"
  | "projects"
  | "agents"
  | "containers";

/** A theme token that tints a section: its text, icon and chart color. */
export type Tint = SectionId | "upload";

export interface Section {
  id: SectionId;
  title: string;
  icon: string;
}

export const SECTIONS: Record<SectionId, Section> = {
  overview: { id: "overview", title: "Overview", icon: LayoutGrid },
  cpu: { id: "cpu", title: "CPU", icon: Cpu },
  memory: { id: "memory", title: "Memory", icon: MemoryStick },
  disk: { id: "disk", title: "Disk", icon: HardDrive },
  network: { id: "network", title: "Network", icon: Globe },
  gpu: { id: "gpu", title: "GPU", icon: Gpu },
  neural: { id: "neural", title: "Neural", icon: Brain },
  battery: { id: "battery", title: "Battery", icon: Battery },
  sound: { id: "sound", title: "Sound", icon: Volume2 },
  bluetooth: { id: "bluetooth", title: "Bluetooth", icon: Bluetooth },
  projects: { id: "projects", title: "Projects", icon: Folder },
  agents: { id: "agents", title: "Agents", icon: Bot },
  containers: { id: "containers", title: "Containers", icon: Package },
};

/** Less frequent destinations, in the title bar's More menu. */
export const MORE_SECTIONS: { heading: string; ids: SectionId[] }[] = [
  { heading: "Device", ids: ["network", "neural", "battery", "sound", "bluetooth"] },
  { heading: "Workspace", ids: ["projects", "agents", "containers"] },
];

// Whole class names, so the build can check them.

/** A tint's text and icon color. */
export const TEXT: Record<Tint, string> = {
  overview: "text-overview",
  cpu: "text-cpu",
  memory: "text-memory",
  disk: "text-disk",
  network: "text-network",
  upload: "text-upload",
  gpu: "text-gpu",
  neural: "text-neural",
  battery: "text-battery",
  sound: "text-sound",
  bluetooth: "text-bluetooth",
  projects: "text-projects",
  agents: "text-agents",
  containers: "text-containers",
};

/** A tint's solid fill. */
export const BG: Record<Tint, string> = {
  overview: "bg-overview",
  cpu: "bg-cpu",
  memory: "bg-memory",
  disk: "bg-disk",
  network: "bg-network",
  upload: "bg-upload",
  gpu: "bg-gpu",
  neural: "bg-neural",
  battery: "bg-battery",
  sound: "bg-sound",
  bluetooth: "bg-bluetooth",
  projects: "bg-projects",
  agents: "bg-agents",
  containers: "bg-containers",
};

/** A tinted tile behind an icon (13%). */
export const TILE: Record<Tint, string> = {
  overview: "bg-overview/13",
  cpu: "bg-cpu/13",
  memory: "bg-memory/13",
  disk: "bg-disk/13",
  network: "bg-network/13",
  upload: "bg-upload/13",
  gpu: "bg-gpu/13",
  neural: "bg-neural/13",
  battery: "bg-battery/13",
  sound: "bg-sound/13",
  bluetooth: "bg-bluetooth/13",
  projects: "bg-projects/13",
  agents: "bg-agents/13",
  containers: "bg-containers/13",
};

/** A tinted chip (12%). */
export const CHIP: Record<Tint, string> = {
  overview: "bg-overview/12",
  cpu: "bg-cpu/12",
  memory: "bg-memory/12",
  disk: "bg-disk/12",
  network: "bg-network/12",
  upload: "bg-upload/12",
  gpu: "bg-gpu/12",
  neural: "bg-neural/12",
  battery: "bg-battery/12",
  sound: "bg-sound/12",
  bluetooth: "bg-bluetooth/12",
  projects: "bg-projects/12",
  agents: "bg-agents/12",
  containers: "bg-containers/12",
};

/** An avatar's tint (17%). */
export const AVATAR: Record<Tint, string> = {
  overview: "bg-overview/17",
  cpu: "bg-cpu/17",
  memory: "bg-memory/17",
  disk: "bg-disk/17",
  network: "bg-network/17",
  upload: "bg-upload/17",
  gpu: "bg-gpu/17",
  neural: "bg-neural/17",
  battery: "bg-battery/17",
  sound: "bg-sound/17",
  bluetooth: "bg-bluetooth/17",
  projects: "bg-projects/17",
  agents: "bg-agents/17",
  containers: "bg-containers/17",
};

/** A meter's track in a tint (13%). */
export const TRACK: Record<Tint, string> = {
  overview: "bg-overview/13",
  cpu: "bg-cpu/13",
  memory: "bg-memory/13",
  disk: "bg-disk/13",
  network: "bg-network/13",
  upload: "bg-upload/13",
  gpu: "bg-gpu/13",
  neural: "bg-neural/13",
  battery: "bg-battery/13",
  sound: "bg-sound/13",
  bluetooth: "bg-bluetooth/13",
  projects: "bg-projects/13",
  agents: "bg-agents/13",
  containers: "bg-containers/13",
};

/** A tint's left-to-right gradient, to its end color where it has one. */
export const GRADIENT: Record<Tint, string> = {
  overview: "bg-linear-to-r from-overview to-overview",
  cpu: "bg-linear-to-r from-cpu to-cpu-end",
  memory: "bg-linear-to-r from-memory to-memory-end",
  disk: "bg-linear-to-r from-disk to-disk-end",
  network: "bg-linear-to-r from-network to-network-end",
  upload: "bg-linear-to-r from-upload to-upload-end",
  gpu: "bg-linear-to-r from-gpu to-gpu-end",
  neural: "bg-linear-to-r from-neural to-neural",
  battery: "bg-linear-to-r from-battery to-battery",
  sound: "bg-linear-to-r from-sound to-sound",
  bluetooth: "bg-linear-to-r from-bluetooth to-bluetooth",
  projects: "bg-linear-to-r from-projects to-projects",
  agents: "bg-linear-to-r from-agents to-agents",
  containers: "bg-linear-to-r from-containers to-containers",
};

/** The theme token a tint's gradients end on. */
export const END: Record<Tint, Tint | "cpu-end" | "memory-end" | "disk-end" | "network-end" | "upload-end" | "gpu-end"> = {
  overview: "overview",
  cpu: "cpu-end",
  memory: "memory-end",
  disk: "disk-end",
  network: "network-end",
  upload: "upload-end",
  gpu: "gpu-end",
  neural: "neural",
  battery: "battery",
  sound: "sound",
  bluetooth: "bluetooth",
  projects: "projects",
  agents: "agents",
  containers: "containers",
};
