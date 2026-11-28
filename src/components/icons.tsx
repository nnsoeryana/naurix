import type { ReactNode } from "react";

/** Small 16px stroke icons (hand-drawn paths, no icon library dependency). */
const base = { width: 16, height: 16, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round", strokeLinejoin: "round", "aria-hidden": true } as const;
const make = (d: ReactNode) => (p: { className?: string }) => <svg {...base} className={p.className}>{d}</svg>;

export const IconHome = make(<><path d="M3 11.5 12 4l9 7.5" /><path d="M5.5 10v9.5h13V10" /><path d="M10 19.5v-5h4v5" /></>);
export const IconFolder = make(<path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h4l2 2h9A1.5 1.5 0 0 1 21 9.5v8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z" />);
export const IconBot = make(<><rect x="4" y="8" width="16" height="11" rx="3" /><path d="M12 8V4.5" /><circle cx="12" cy="3.7" r="1" /><circle cx="9" cy="13" r="1.2" /><circle cx="15" cy="13" r="1.2" /><path d="M2.5 13v2M21.5 13v2" /></>);
export const IconPlug = make(<><path d="M9 3v5M15 3v5" /><path d="M6.5 8h11v3.5a5.5 5.5 0 0 1-11 0z" /><path d="M12 17v4" /></>);
export const IconBolt = make(<path d="M13 3 5 13.5h6L10 21l8-10.5h-6z" />);
export const IconFlow = make(<><circle cx="5.5" cy="6" r="2" /><circle cx="5.5" cy="18" r="2" /><circle cx="18.5" cy="12" r="2" /><path d="M7.5 6h3a3 3 0 0 1 3 3v0M7.5 18h3a3 3 0 0 0 3-3v0M13.5 9a3 3 0 0 0 3 3h0M13.5 15a3 3 0 0 1 3-3" /></>);
export const IconNote = make(<><path d="M6 3.5h9l3.5 3.5v13.5H6z" /><path d="M15 3.5V7h3.5M9 12h6M9 15.5h6" /></>);
export const IconShield = make(<><path d="M12 3 4.5 6v5.5c0 4.6 3.2 8.2 7.5 9.5 4.3-1.3 7.5-4.9 7.5-9.5V6z" /><path d="m9 12 2 2 4-4" /></>);
export const IconGit = make(<><circle cx="6.5" cy="5.5" r="2" /><circle cx="6.5" cy="18.5" r="2" /><circle cx="17.5" cy="9" r="2" /><path d="M6.5 7.5v9" /><path d="M17.5 11c0 3.5-4 3.5-9.5 6" /></>);
export const IconGear = make(<><circle cx="12" cy="12" r="3" /><path d="M12 2.5v2.6M12 18.9v2.6M2.5 12h2.6M18.9 12h2.6M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M18.7 5.3l-1.8 1.8M7.1 16.9l-1.8 1.8" /></>);
export const IconPlus = make(<path d="M12 5v14M5 12h14" />);
export const IconClose = make(<path d="M6 6l12 12M18 6 6 18" />);
export const IconPencil = make(<path d="M4 20l1-4L16.5 4.5a2 2 0 0 1 3 3L8 19z" />);
export const IconStop = make(<rect x="6" y="6" width="12" height="12" rx="2" />);
