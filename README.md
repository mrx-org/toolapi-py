# toolapi-py

> **Disclaimer:** This Python wrapper was generated using [Claude Code](https://claude.ai/code). The underlying [toolapi](https://github.com/mrx-org/toolapi) Rust crate it wraps is fully human-written.

Python bindings for the `toolapi` Rust crate, built with [PyO3](https://pyo3.rs) and [Maturin](https://www.maturin.rs).

## Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs))
- Python >= 3.8
- [maturin](https://www.maturin.rs) and optionally [uv](https://github.com/astral-sh/uv)

## Building

### With uv (recommended)

```bash
uv pip install maturin
maturin develop
```

### With maturin directly

```bash
pip install maturin
maturin develop
```

### Release build

```bash
maturin build --release --out dist --find-interpreter
```

## Testing

Running with uv will automatically re-build the toolapi-py dependency:

```bash
uv run test_toolapi.py
```

## Installation

Currently this package must be built locally. In the future it may be published to PyPI, using a GitHub Actions workflow generated with:

```bash
maturin generate-ci github
```
