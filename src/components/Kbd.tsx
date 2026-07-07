import type { ReactNode } from "react";
import { color, font } from "../theme";

/** A single keycap, as shown for the panel shortcut in settings. */
export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd
      style={{
        background: color.keycap,
        border: `1px solid ${color.hairStrong}`,
        borderBottomWidth: 2,
        borderRadius: 6,
        padding: "4px 8px",
        fontFamily: font.mono,
        fontSize: 12,
        fontWeight: 600,
        color: color.ink,
      }}
    >
      {children}
    </kbd>
  );
}
