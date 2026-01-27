# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`toolapi` is a Python package with a Rust extension module built using PyO3 and Maturin. It wraps the `toolapi` Rust crate (from `github.com/mrx-org/toolapi`) into a Python-importable module.

## Build Commands

```bash
# Development build (requires maturin and Rust toolchain)
maturin develop

# Release build producing wheel artifacts
maturin build --release --out dist --find-interpreter

# Source distribution
maturin sdist --out dist
```

No test suite or linter is currently configured.

## Architecture

- **`src/lib.rs`** — Rust library compiled as `_core` cdylib via PyO3. This is the extension module entry point. Functions defined here are exposed to Python.
- **`src/toolapi/__init__.py`** — Python package that re-exports from the compiled `_core` extension.
- **`src/toolapi/_core.pyi`** — Type stubs for the Rust extension, providing IDE autocomplete and type checker support.

The Rust side depends on an external `toolapi` crate (git dependency) and `pyo3` with ABI3 stable API targeting Python 3.9+. The Python side requires `>=3.8`.

Build system: Maturin (`pyproject.toml` build-backend) with uv for Python environment management. CI uses `maturin generate-ci github` workflow building wheels for Linux/Windows/macOS across multiple architectures.
