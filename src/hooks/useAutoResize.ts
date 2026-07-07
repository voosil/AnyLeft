/**
 * Resize the current Tauri window to hug a content element.
 *
 * The panel is a borderless, transparent window; sizing it to the card keeps
 * the dropdown tight regardless of how many providers are connected. Outside
 * Tauri this is a no-op.
 */

import { useLayoutEffect } from "react";
import { isTauri } from "../api/bridge";

export function useAutoResize(
  ref: React.RefObject<HTMLElement | null>,
  deps: React.DependencyList,
): void {
  useLayoutEffect(() => {
    if (!isTauri()) return;
    const el = ref.current;
    if (!el) return;

    let cancelled = false;
    const apply = async () => {
      const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
      if (cancelled) return;
      const width = Math.ceil(el.offsetWidth);
      const height = Math.ceil(el.offsetHeight);
      await getCurrentWindow().setSize(new LogicalSize(width, height));
    };

    // Wait a frame so fonts/layout settle before measuring.
    const raf = requestAnimationFrame(() => void apply());
    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
