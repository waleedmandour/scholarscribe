// Icon name registry, shared by Icon.svelte and its consumers.
// Kept in a plain TS module because Svelte 5 does not allow `export type`
// from instance scripts (svelte-check rejects the modifier).

export type IconName =
  | "cube" | "info" | "eraser" | "quote" | "chart" | "layers" | "file-text"
  | "compass" | "message" | "book" | "gauge" | "pen" | "fingerprint"
  | "wave" | "history" | "shield" | "mail" | "file-check" | "eye" | "save"
  | "sun" | "moon" | "monitor" | "sparkle" | "search" | "refresh"
  | "check" | "x" | "arrow-right" | "alert" | "cpu" | "server"
  | "spellcheck" | "plug";

export interface IconPath {
  d?: string;
  extra?: string;
}

export const iconPaths: Record<IconName, IconPath[]> = {
  cube: [{ d: "M12 2.7 20.5 7.4 M12 2.7 3.5 7.4 M12 2.7v0 M20.5 7.4v9.2L12 21.3 3.5 16.6V7.4L12 2.7 M12 11.9 20.5 7.4 M12 11.9v9.4 M12 11.9 3.5 7.4" }],
  info: [{ d: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z" }, { d: "M12 11v5" }, { extra: '<circle cx="12" cy="8" r="0.4" fill="currentColor" stroke="none"/>' }],
  eraser: [{ d: "M7.5 20.5h12 M18.4 13.7 10.3 21.8a2 2 0 0 1-2.8 0l-4.2-4.2a2 2 0 0 1 0-2.8l8.6-8.6a2 2 0 0 1 2.8 0l5 5a2 2 0 0 1 0 2.8l-1.3 1.3Z" }, { d: "m8.5 8.5 7 7" }],
  quote: [{ d: "M9.5 7.5c-2.6.7-4 2.4-4 5v4h4.5v-4.5H7.3c0-1.7.7-2.7 2.2-3.2Z M18.5 7.5c-2.6.7-4 2.4-4 5v4H19v-4.5h-2.7c0-1.7.7-2.7 2.2-3.2Z" }],
  chart: [{ d: "M4 4v16h16" }, { d: "M8.5 16.5v-5 M12.5 16.5V8 M16.5 16.5v-7.5" }],
  layers: [{ d: "m12 3.5 8.5 4.5-8.5 4.5L3.5 8Z" }, { d: "m3.5 12.5 8.5 4.5 8.5-4.5 M3.5 16.5l8.5 4.5 8.5-4.5" }],
  "file-text": [{ d: "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z" }, { d: "M14 3v5h5 M9.5 12.5h5 M9.5 16h5" }],
  compass: [{ d: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z" }, { d: "m15.5 8.5-2 5-5 2 2-5Z" }],
  message: [{ d: "M20.5 11.5a7.8 7.8 0 0 1-8.2 7.7 9 9 0 0 1-3.1-.5L4 20l1.3-4.1a7.4 7.4 0 0 1-1.3-4.2 7.8 7.8 0 0 1 8.3-7.7 7.8 7.8 0 0 1 8.2 7.5Z" }, { d: "M9 11.5h.01 M12.5 11.5h.01 M16 11.5h.01" }],
  book: [{ d: "M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5Z" }, { d: "M4 20.5V5.5 M4 13h4.5 M20 18v3H6.5 M8.5 7.5h7 M8.5 11h4" }],
  gauge: [{ d: "M4.5 18.5a9 9 0 1 1 15 0" }, { d: "M12 15.5 15.5 9" }, { extra: '<circle cx="12" cy="15.5" r="1.2" fill="currentColor" stroke="none"/>' }],
  pen: [{ d: "m14.5 4.5 5 5L8 21H3v-5Z" }, { d: "m12.5 6.5 5 5" }],
  fingerprint: [{ d: "M6 8.5a8 8 0 0 1 12 0 M8.5 12a4.5 4.5 0 0 1 7 0 M11 15.5a1.6 1.6 0 0 1 2 0 M12 19v.01 M6 8.5c-.9 1.4-1.4 3-1.4 4.8 0 1.4.2 2.5.7 3.7 M18 8.5c.9 1.4 1.4 3 1.4 4.8 0 2.5-.4 4.3-1.4 6" }],
  wave: [{ d: "M3 12h2 M7 12h.01 M8 8.5v7 M11.5 5.5v13 M15 7.5v9 M18.5 10v4 M21 11.5v1" }],
  history: [{ d: "M4 12a8 8 0 1 1 2.3 5.6" }, { d: "M4 13V8.5 M4 13h4.5 M12 8v4.2l3 1.8" }],
  shield: [{ d: "M12 3 5 5.8v5.4c0 4.4 3 7.7 7 9.8 4-2.1 7-5.4 7-9.8V5.8Z" }, { d: "m9 11.5 2.2 2.2 4-4.4" }],
  mail: [{ d: "M4 6.5h16v11H4Z" }, { d: "m4 7.5 8 5.5 8-5.5" }],
  "file-check": [{ d: "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8Z" }, { d: "M14 3v5h5" }, { d: "m9.5 14.5 1.8 1.8 3.4-3.8" }],
  eye: [{ d: "M3 12s3.5-6 9-6 9 6 9 6-3.5 6-9 6-9-6-9-6Z" }, { extra: '<circle cx="12" cy="12" r="2.6"/>' }],
  save: [{ d: "M5 3.5h11L20.5 8v11a1.5 1.5 0 0 1-1.5 1.5H5A1.5 1.5 0 0 1 3.5 19V5A1.5 1.5 0 0 1 5 3.5Z" }, { d: "M8 3.5V9h7V3.5 M7.5 20.5v-6h9v6" }],
  sun: [{ extra: '<circle cx="12" cy="12" r="4"/>' }, { d: "M12 2.5v2 M12 19.5v2 M2.5 12h2 M19.5 12h2 M5.3 5.3l1.4 1.4 M17.3 17.3l1.4 1.4 M18.7 5.3l-1.4 1.4 M6.7 17.3l-1.4 1.4" }],
  moon: [{ d: "M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5Z" }],
  monitor: [{ d: "M4 4.5h16v11H4Z" }, { d: "M9 20.5h6 M12 15.5v5" }],
  sparkle: [{ d: "M12 3.5c.6 3.9 2.6 5.9 6.5 6.5-3.9.6-5.9 2.6-6.5 6.5-.6-3.9-2.6-5.9-6.5-6.5 3.9-.6 5.9-2.6 6.5-6.5Z" }, { d: "M19 14.5c.3 1.9 1.2 2.8 3 3-1.8.3-2.7 1.2-3 3-.3-1.8-1.2-2.7-3-3 1.8-.2 2.7-1.1 3-3Z" }],
  search: [{ extra: '<circle cx="11" cy="11" r="6.5"/>' }, { d: "m16 16 4.5 4.5" }],
  refresh: [{ d: "M20 12a8 8 0 1 1-2.3-5.6" }, { d: "M20 3.5V8h-4.5" }],
  check: [{ d: "m5 12.5 4.5 4.5L19 7.5" }],
  x: [{ d: "m6 6 12 12 M18 6 6 18" }],
  "arrow-right": [{ d: "M4 12h15 M13.5 6.5 19.5 12l-6 5.5" }],
  alert: [{ d: "M12 4 2.8 19.5h18.4Z" }, { d: "M12 10v4.2" }, { extra: '<circle cx="12" cy="17" r="0.4" fill="currentColor" stroke="none"/>' }],
  cpu: [{ extra: '<rect x="6" y="6" width="12" height="12" rx="1.5"/>' }, { extra: '<rect x="9.5" y="9.5" width="5" height="5" rx="0.5"/>' }, { d: "M9 2.5V6 M12 2.5V6 M15 2.5V6 M9 18v3.5 M12 18v3.5 M15 18v3.5 M2.5 9H6 M2.5 12H6 M2.5 15H6 M18 9h3.5 M18 12h3.5 M18 15h3.5" }],
  server: [{ extra: '<rect x="3.5" y="4" width="17" height="7" rx="1.5"/>' }, { extra: '<rect x="3.5" y="13" width="17" height="7" rx="1.5"/>' }, { d: "M7 7.5h.01 M7 16.5h.01" }],
  spellcheck: [{ d: "M4 5.5h16 M4 10.5h9.5 M4 15.5h6" }, { d: "m13.5 17.5 2.2 2.2 4.3-4.9" }],
  plug: [
    { d: "M9 3.5v4 M15 3.5v4 M7 7.5h10v4a5 5 0 0 1-10 0Z M12 16.5v4" },
  ],
};
