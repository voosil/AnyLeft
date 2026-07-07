/**
 * Design tokens ported from the AnyLeft design files.
 *
 * Every color, font stack, and gradient the UI uses lives here so screens and
 * components never hardcode raw values. Names describe role, not appearance.
 */

export const font = {
  sans: "'Schibsted Grotesk','Noto Sans SC',-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif",
  mono: "'IBM Plex Mono',ui-monospace,SFMono-Regular,Menlo,monospace",
} as const;

export const color = {
  // Ink / text
  ink: "#33302A",
  inkStrong: "#2E2A22",
  inkSoft: "#5F564A",
  muted: "#8a8072",
  faint: "#9A8F80",
  brown: "#9A5A34",

  // Brand / status
  accent: "#C96442",
  green: "#6E8A4E",
  gold: "#E4B24A",
  warn: "#B4831F",
  link: "#274A80",

  // Surfaces
  cardGlass: "rgba(247,242,231,.95)",
  card: "#F7F2E7",
  inner: "#FCF9F2",
  titlebarTop: "#EFE8D9",
  titlebarBottom: "#E8DFCB",
  keycap: "#EFE8D9",

  // Menu bar (dark)
  menubar: "rgba(48,41,31,.82)",
  menubarText: "#EAE3D4",

  // Traffic lights
  trafficRed: "#FF5F57",
  trafficAmber: "#FEBC2E",
  trafficGreen: "#28C840",

  // Lines / hairlines
  hair: "rgba(51,48,42,.1)",
  hairSoft: "rgba(51,48,42,.07)",
  hairStrong: "rgba(51,48,42,.14)",
  highlightBorder: "rgba(255,255,255,.6)",
  toggleOff: "rgba(51,48,42,.18)",
  chip: "rgba(51,48,42,.06)",
} as const;

/** The warm parchment app background (used for full-screen / preview surfaces). */
export const appGradient =
  "radial-gradient(1200px 720px at 18% -14%,rgba(228,178,74,.30),transparent 60%)," +
  "radial-gradient(960px 640px at 104% 24%,rgba(39,74,128,.24),transparent 62%)," +
  "linear-gradient(158deg,#EFE7D6,#E2D3B8 58%,#D5C09C)";

/** Percentage at/above which a provider is "near its limit". Mirrors Rust. */
export const NEAR_LIMIT_THRESHOLD = 85;
