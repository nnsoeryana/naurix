# Naurix companion

A small animated aurora card at the bottom of the sidebar. It is decorative and informative only: it has no controls, never takes focus and never starts, stops or approves anything.

## Visual design
An original ribbon in the Naurix cyan / blue / violet, drawn as inline SVG (`features/nauri/components/NauriVisual.tsx`). Colors are theme tokens (`--color-teal`, `--color-info`, `--color-purple`, `--color-terminal`, `--color-warning`), so every built-in theme keeps the identity. Only the active state is in the DOM. Tagline: "Your Coding Companion" (card tooltip).

## States
| State | Look | Motion |
|---|---|---|
| Idle | one soft ribbon, aura | slow float, breathing |
| Ready | same ribbon as idle | even slower float; an agent is alive but went quiet ("Your turn!") |
| Working | five flowing ribbons | seamless horizontal flow |
| Waiting | thin wisp, motes | slow pulse, light gathers to the centre |
| Success | radial burst, sparkles | one burst, about 2.4 s, then back to idle |
| Error | dimmer ribbon + warning line | very slow sway, no flashing |

## Agent status mapping (honest by design)
Naurix reads the existing `paneState` / `paneKinds` in the store; it has no lifecycle of its own (`features/nauri/state.ts`). Per AGENTS.md and `docs/AGENT_ADAPTERS.md`, Naurix has no reliable task-level signal yet, so only **process** facts are used:

| Evidence | Naurix |
|---|---|
| no agent pane alive | Idle |
| agent `starting` / `running`, output flowing after the user's Enter or the launch | Working, text says "Waiting for your agent's response..." (a heuristic, not an agent signal) |
| agent alive, no output for 3.5 s (`QUIET_MS` in `activity.ts`) | Ready |
| `failed`, or exit with non-zero code | Error |
| exit by signal (user stopped it) | nothing |
| `running` -> exit code 0 observed live | one Success burst |
| a reliable "needs input" adapter signal | Waiting. Wired (`summarize(..., waitingPanes)`) but **no adapter supplies it yet**, so it never shows |

Success is an event, not a state: it fires only on a live running -> clean-exit transition, so restoring state, stopping an agent or a failure never plays it. Success text is "finished cleanly", not "task completed".

## Multiple agents
All agent panes in all workspaces are counted. Priority: waiting > error > working > ready > idle. Turn activity (`features/nauri/activity.ts`) holds only pane ids and timers, never terminal content. Success is shown only when nothing outranks it. Text: "Waiting for 3 agents...", "2 agents ended with errors."

## Performance
- Only `transform` and `opacity` animate; no filters, blur, canvas, WebGL, JS animation loops or extra WebView.
- About 10-40 SVG nodes, one state at a time.
- Animation pauses while the window is hidden (`visibilitychange`).
- A disabled Naurix renders nothing and holds no store subscription or timer.

## Reduced motion and settings
Settings > General: show Naurix (default on), animations (default on), reduced animation (about 2x slower). macOS Reduce Motion (`prefers-reduced-motion`) always gives the static visual. Stored in `general.nauriEnabled / nauriAnimations / nauriReduced` (`crates/core/src/settings.rs`; old settings files load with the defaults).

## Adding a state
1. Add it to `NauriMode` in `types.ts`.
2. Map it in `summarize` / `displayMode` / `statusText` in `state.ts`, with tests, and only from a reliable signal.
3. Add its `{mode === "..." && <g>}` block in `NauriVisual.tsx` and its keyframes in `animations/nauri.css` (transform/opacity only; the base style must look right with animation off).
