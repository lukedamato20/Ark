/**
 * UX: the single source of truth for Settings' navigation categories — used by the nav itself and
 * by `SettingsView`'s section-content switch, so a category can never exist in the nav without
 * content or vice versa. Deliberately only lists categories backed by real, already-existing
 * functionality (no empty "General"/"About" placeholders for future features — see the category
 * audit that produced this exact list).
 */
export type SettingsSectionId =
  "ai-behavior" | "providers" | "models" | "appearance" | "shortcuts" | "storage" | "privacy" | "advanced";

export interface SettingsSectionMeta {
  id: SettingsSectionId;
  label: string;
  description: string;
}

export const SETTINGS_SECTIONS: SettingsSectionMeta[] = [
  {
    id: "ai-behavior",
    label: "AI & Behavior",
    description: "Personas, projects, and how generation settings are resolved.",
  },
  {
    id: "providers",
    label: "Providers",
    description: "Connect and configure model providers, credentials, and the built-in runtime.",
  },
  {
    id: "models",
    label: "Models",
    description: "Install, update, and remove local Ollama models.",
  },
  {
    id: "appearance",
    label: "Appearance",
    description: "Theme.",
  },
  {
    id: "shortcuts",
    label: "Keyboard Shortcuts",
    description: "Every shortcut Ark currently supports.",
  },
  {
    id: "storage",
    label: "Storage & Data",
    description: "Workspace location, encryption, backups, and import/export.",
  },
  {
    id: "privacy",
    label: "Privacy & Security",
    description: "Credential storage, tool capability grants, and the local companion API.",
  },
  {
    id: "advanced",
    label: "Advanced",
    description: "Diagnostics and benchmarking.",
  },
];

export const DEFAULT_SETTINGS_SECTION: SettingsSectionId = "ai-behavior";
