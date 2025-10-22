# Contributing to AkiDB

We welcome contributions that make AkiDB faster, safer, and easier to operate. The guidelines below help us keep the project maintainable and production-ready.

## Ground Rules
- Automate everything you can and document automation gaps.
- Every change must have tests or a clear rationale for skipping them.
- Keep the main branch deployable at all times.
- Security, observability, and reliability are first-class concerns.

## Getting Started
1. Fork the repository and create a feature branch (`git checkout -b feat/my-improvement`).
2. Bootstrap the local toolchain (Rust stable) and Docker.
3. Follow the steps in [docs/development-setup.md](development-setup.md) to provision the development stack.
4. Run `./scripts/dev-test.sh` before pushing any change.

## Coding Standards
- Rust edition 2021.
- Enforce style with `cargo fmt` and lint with `cargo clippy -- -D warnings`.
- Prefer small, composable modules with explicit error handling.
- Write benchmarks behind feature flags whenever possible.

## Commit & PR Guidelines
- Commit messages: `<type>(scope): summarise change` (e.g. `feat(storage): add snapshot pruning`).
- Keep commits focused; break large features into logical pieces.
- Reference issues or RFCs in commit messages and PR descriptions when applicable.
- Include screenshots or terminal output for operational changes when helpful.

## Testing Expectations
- Unit tests for new logic.
- Integration tests when touching cross-crate behaviour.
- Run benchmarks (`cargo test --benches`) for components that affect performance.
- CI must be green before requesting review.

## Code Review Checklist
- Does the change handle error cases safely?
- Are monitoring/metrics updates required?
- Will the deployment pipeline need adjustments?
- Is documentation (user or operator docs) updated?

## Release Readiness
- Update `docs/development-setup.md` or other operator docs when behaviour changes.
- Ensure automation scripts continue to work (`scripts/dev-init.sh`, `scripts/build-release.sh`).
- Communicate migrations or manual steps in the PR description.

Automate everything, monitor everything, break nothing. Thanks for helping build a resilient AkiDB!
