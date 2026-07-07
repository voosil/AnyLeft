import { color } from "../theme";

interface ToggleProps {
  on: boolean;
  onToggle: () => void;
  label?: string;
}

/** The pill switch used for accounts and preferences. */
export function Toggle({ on, onToggle, label }: ToggleProps) {
  return (
    <span
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={onToggle}
      style={{
        width: 38,
        height: 22,
        borderRadius: 99,
        background: on ? color.green : color.toggleOff,
        position: "relative",
        flex: "none",
        cursor: "pointer",
        transition: "background .18s",
      }}
    >
      <span
        style={{
          position: "absolute",
          top: 2,
          left: on ? undefined : 2,
          right: on ? 2 : undefined,
          width: 18,
          height: 18,
          borderRadius: "50%",
          background: "#fff",
          boxShadow: "0 1px 2px rgba(0,0,0,.25)",
          transition: "left .18s, right .18s",
        }}
      />
    </span>
  );
}
