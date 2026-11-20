# Naurix website

Marketing site for Naurix. Next.js (App Router), TypeScript (strict), Tailwind CSS v4, Lucide. Static output, no backend, no telemetry. Independent from the desktop app (own `package.json`).

## Run

```sh
cd website
pnpm install
pnpm dev          # http://localhost:3000
pnpm typecheck && pnpm lint && pnpm build
```

TypeScript is pinned to 6.x because `typescript-eslint` does not support 7.x yet.

## Where to edit

| What | File |
|---|---|
| All external URLs, release link, nav, primary CTA, hero screenshot | `src/config/site.ts` |
| Feature list, status (available / in-development / planned), agent list | `src/config/features.ts` |
| Colors, fonts | `src/app/globals.css` (`@theme`) |
| Aurora / Naurix motion | `src/styles/animations.css` |
| Sections | `src/components/sections/` |

## Honesty rules

- Feature status comes from the repository README ("What works today"). Update `features.ts` when the product changes.
- `urls.release` is `null` until a real installer is published; the site then shows "Build from source" instead of "Download Naurix".
- No Apple Silicon support or performance claims: only an Intel run is recorded (see `docs/PERFORMANCE_AND_COMPATIBILITY.md`).
- The Naurix animation is a brand illustration, not tied to agent state.
- No fabricated stars, contributors or download counts.

## Needs owner input

- Real app screenshot: add `public/screenshots/app.png`, set `screenshot` in `site.ts` (placeholder shows until then).
- Release/download URL (`urls.release`).
- Docs site URL, if one is created (currently links to `docs/` on GitHub).
- Production origin defaults to `https://naurix.nana-dev.web.id` (override with `NEXT_PUBLIC_SITE_URL`). Still needed: an Open Graph image.
- Confirm the GitHub repo is public (links assume `github.com/nnsoeryana/naurix`, `main`).
