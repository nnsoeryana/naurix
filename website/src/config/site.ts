/**
 * Central site + URL configuration.
 * Every external URL lives here. Set a value to `null` to hide the action that needs it.
 * Only URLs that exist today are filled in; nothing is invented.
 */
const GITHUB_REPO = "https://github.com/nnsoeryana/naurix";

export const site = {
  name: "Naurix",
  tagline: "Think freely. Build boldly. Stay in control.",
  description:
    "Naurix is an open-source, lightweight terminal and workspace for AI coding agents. Manage terminals, agents, MCP integrations and skills in one place, locally, on macOS.",
  year: new Date().getFullYear(),
  license: "Apache-2.0",
  /** Public origin of the website (override with NEXT_PUBLIC_SITE_URL, e.g. for previews). */
  origin: process.env.NEXT_PUBLIC_SITE_URL ?? "https://naurix.nana-dev.web.id",
} as const;

export const urls = {
  github: GITHUB_REPO,
  /** Docs live in the repository for now; no separate docs site exists. */
  docs: `${GITHUB_REPO}/tree/main/docs`,
  roadmap: `${GITHUB_REPO}/blob/main/docs/IMPLEMENTATION_PLAN.md`,
  contributing: `${GITHUB_REPO}/blob/main/CONTRIBUTING.md`,
  license: `${GITHUB_REPO}/blob/main/LICENSE`,
  security: `${GITHUB_REPO}/blob/main/SECURITY.md`,
  /** Build-from-source instructions (no packaged release exists yet). */
  buildFromSource: `${GITHUB_REPO}#quick-start`,
  /**
   * A real release/installer URL. `null` = no signed release is published yet,
   * so the site shows "Build from source" instead of a download button.
   */
  release: null as string | null,
} as const;

export const nav = [
  { label: "Product", href: "#product" },
  { label: "Features", href: "#features" },
  { label: "Naurix", href: "#naurix" },
  { label: "Open Source", href: "#open-source" },
  { label: "Docs", href: urls.docs, external: true },
] as const;

/** Primary call-to-action, resolved once so Nav, Hero and Final CTA stay consistent. */
export const primaryCta = urls.release
  ? { label: "Download Naurix", href: urls.release, download: true }
  : { label: "Build from source", href: urls.buildFromSource, download: false };

/** Hero product screenshot (compressed WebP; the original PNG is kept in `design/`). `null` shows the placeholder. */
export const screenshot: { src: string; alt: string; width: number; height: number } | null = {
  src: "/screenshots/app.webp",
  alt: "Naurix desktop workspace: a project sidebar, a Claude Code session on the left and a terminal running pnpm dev on the right.",
  width: 1586,
  height: 992,
};
