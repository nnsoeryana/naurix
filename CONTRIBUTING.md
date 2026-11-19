# Contributing

1. Read `docs/PRODUCT_VISION.md`, `docs/PRD.md`, `docs/TECHNICAL_DESIGN.md`, and `AGENTS.md`.
2. Reference PRD requirement IDs in issues and PRs. Keep changes small and tied to a milestone in `docs/IMPLEMENTATION_PLAN.md`.
3. Significant decisions need an ADR in `docs/adr/`.
4. Before opening a PR: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and the frontend lint/type-check/test scripts (added in M1).
5. Do not add dependencies without checking maintenance, license, macOS x86_64+arm64 support, and size impact.
6. Never commit secrets or terminal/agent transcripts. No telemetry without an approved opt-in design.
7. Claims about Intel/Apple Silicon or performance must cite a recorded run.

Contributions are licensed under Apache-2.0. Please sign off commits (`git commit -s`).
