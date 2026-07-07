import { font } from "../theme";

interface ProviderBadgeProps {
  mono: string;
  accent: string;
  tint: string;
  size?: number;
}

/** The rounded monogram badge (e.g. "GPT", "C") used across the settings UI. */
export function ProviderBadge({ mono, accent, tint, size = 30 }: ProviderBadgeProps) {
  return (
    <span
      style={{
        width: size,
        height: size,
        borderRadius: size >= 38 ? 10 : 9,
        background: tint,
        color: accent,
        display: "grid",
        placeItems: "center",
        fontFamily: font.mono,
        fontWeight: 600,
        fontSize: size >= 38 ? 13 : 11,
        flex: "none",
      }}
    >
      {mono}
    </span>
  );
}
