"use client";

import { useEffect } from "react";

/**
 * One tiny client island for the whole page:
 * - reveals `.reveal` elements once as they scroll into view
 * - pauses continuous animations while the tab is hidden
 * Content is visible by default if JS or IntersectionObserver is unavailable (see animations.css).
 */
export function RevealObserver() {
  useEffect(() => {
    const els = document.querySelectorAll<HTMLElement>(".reveal");
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let io: IntersectionObserver | undefined;

    if (reduce || !("IntersectionObserver" in window)) {
      els.forEach((el) => el.classList.add("is-visible"));
    } else {
      io = new IntersectionObserver(
        (entries) => {
          for (const e of entries) {
            if (e.isIntersecting) {
              e.target.classList.add("is-visible");
              io?.unobserve(e.target);
            }
          }
        },
        { rootMargin: "0px 0px -8% 0px", threshold: 0.08 },
      );
      els.forEach((el) => io?.observe(el));
    }

    const onVis = () => document.documentElement.classList.toggle("paused", document.hidden);
    document.addEventListener("visibilitychange", onVis);
    return () => {
      io?.disconnect();
      document.removeEventListener("visibilitychange", onVis);
    };
  }, []);

  return null;
}
