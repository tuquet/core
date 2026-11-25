# Core — Rust Engine Directives

> **Contracts:** [`./CONSTRAINTS.md`](./CONSTRAINTS.md) · Root directives & glossary: [`../AGENTS.md`](../AGENTS.md), [`../docs/GLOSSARY.md`](../docs/GLOSSARY.md). Never weaken or bypass assertions.

## 1. Scope & Subsystems

Device-local execution plane (`tuquet/core` submodule under `operon/core`):

| Crate | Path | Role |
| :--- | :--- | :--- |
| **cli** (`opn`) | `./crates/cli` | Master binary (`opn`), local `daemon` API (`:8765`), automation runner. |
| **worker** | `./crates/worker` | Process supervisor & cloud `SKIP LOCKED` claim loop. |
| **browser** | `./crates/browser` | Stealth Chromium launcher, CDP driver, `browser profile` isolation. |

## 2. Verification

Run inside `core/` (or from umbrella root with `--manifest-path core/Cargo.toml`):
- Quick check: `cargo check --workspace --all-targets`
- Local CI parity:
  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace --lib --bins --tests
  ```

## 3. Guardrails

- **Minimal change:** Smallest diff that solves the task; never add shims or fallback chains to hide schema drift.
- **Evidence-first:** Only report passing checks you ran after the final edit.
- **Commit & push:** Only commit locally (`git commit --no-verify`); never push without explicit user instruction.
- **VPS limits:** **NEVER** run `cargo build --release` on host; clean `target/` if disk $> 85\%$.
