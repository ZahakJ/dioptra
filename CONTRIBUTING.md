# Contributing to Dioptra

Thank you for considering it. Dioptra is a statistics tool, so correctness comes first:
every numerical change needs a reference and a test against a known value.

## Before you start

- Read `DESIGN.md` (the principles) and `docs/METHODS.md` (where every method's reference
  is recorded).
- Search the issues; open one for anything larger than a typo so the approach can be agreed.

## Setting up

```sh
git clone https://github.com/ZahakJ/dioptra && cd dioptra
rustup toolchain install 1.99.0 && rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cd apps/dioptra && pnpm install && cd ../..
./verify --quick        # fmt, clippy, cargo tests, wasm build, tsc, vitest
./verify                # plus frontend build, desktop build, Playwright e2e
```

Linux needs the webkit2gtk-4.1, gtk3 and libsoup3 development packages (see `ci.yml`).

## Rules for numerical code

1. Name the reference (paper, algorithm, or library whose convention you follow) in the doc
   comment and in `docs/METHODS.md`.
2. Add or extend a fixture: `scripts/oracle/gen_fixtures.py` produces
   `fixtures/oracle/*.json` from SciPy, statsmodels, lifelines and scikit-learn; NIST sets
   live in `fixtures/nist/`. Tests are data-driven and compare every field at the stored
   tolerance.
3. Record disagreements with Prism or SciPy conventions explicitly; do not silently pick one.
4. `cargo clippy -D warnings` and `cargo fmt` are part of the gate.

## Rules for the UI

- Keyboard first: every action reachable from the command palette with a shortcut.
- Both themes, no colour-only state, visible focus.
- UI strings in English; numbers use Western digits.

## Pull requests

- One topic per PR, with tests.
- Describe what changed and why; link the issue.
- `./verify` must be green; CI runs it on every push.

## Reporting a numerical discrepancy

Open an issue with the data (or a minimal reproduction), the result Dioptra gave, the
result you expected and where it came from (R output, a textbook, Prism). These are the
most valuable issues this project can receive.
