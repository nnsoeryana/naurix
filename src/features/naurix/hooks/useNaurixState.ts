import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { getBusyPanes, subscribeActivity } from "../activity";
import { useStore } from "../../../state/store";
import { detectCompletions, displayMode, motionLevel, statusText, summarize, SUCCESS_MS } from "../state";
import type { MotionLevel, NaurixMode } from "../types";

/** Derives Naurix's mode from the existing agent pane state; owns no agent state of its own. */
export function useNaurixState(): { mode: NaurixMode; text: string } {
  const states = useStore((s) => s.paneState);
  const kinds = useStore((s) => s.paneKinds);
  const busy = useSyncExternalStore(subscribeActivity, getBusyPanes);
  const summary = useMemo(() => summarize(states, kinds, undefined, busy), [states, kinds, busy]);
  const prev = useRef(states);
  const timer = useRef<number>(0);
  const [celebrating, setCelebrating] = useState(false);

  useEffect(() => {
    const done = detectCompletions(prev.current, states, kinds);
    prev.current = states;
    if (done.length === 0) return;
    setCelebrating(true);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setCelebrating(false), SUCCESS_MS);
  }, [states, kinds]);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const mode = displayMode(summary, celebrating);
  return { mode, text: statusText(mode, summary) };
}

/** Settings + system "reduce motion" + window visibility -> how much Naurix may move. */
export function useNaurixMotion(animations: boolean, reduced: boolean): MotionLevel {
  const [systemReduced, setSystemReduced] = useState(() => window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  const [hidden, setHidden] = useState(() => document.hidden);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onMq = () => setSystemReduced(mq.matches);
    const onVis = () => setHidden(document.hidden);
    mq.addEventListener("change", onMq);
    document.addEventListener("visibilitychange", onVis);
    return () => {
      mq.removeEventListener("change", onMq);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, []);
  return motionLevel({ animations, reduced, systemReduced, hidden });
}
