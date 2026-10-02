# ck (`ck-lite`)

Fast, lightweight regex code search tool with gitignore awareness.

A fork of [`ck`](https://github.com/BeaconBay/ck), simplified to pure regex and literal search without machine learning or embedding runtimes.

## Installation

```bash
cargo install ck-lite
```

This installs the `ck` binary.

## Features

- Fast regex and literal pattern matching across files and directories.
- Respects `.gitignore` and `.ckignore` rules by default.
- Rayon-powered multithreaded directory traversal.
- Grep-compatible flags (`-i`, `-w`, `-F`, `-n`, `-r`, `-C`, `-A`, `-B`, `-l`, `-L`, `--hidden`).
- Context line preview with ANSI match highlighting.
- JSON and JSONL output options for scripting and tool integration.

## Usage

```bash
# Basic regex search
ck "pattern" .

# Case-insensitive search with line numbers
ck -in "pattern" src/

# Fixed literal string match
ck -F "fn main()" .

# Context lines (before and after)
ck -C 2 "error" .

# JSON / JSONL output
ck --json "pattern" .
ck --jsonl "pattern" .

# Include hidden files
ck --hidden "pattern" .

# Bypass ignore files
ck --no-ignore "pattern" .
```

## Upstream

`ck-lite` is a radical simplification of [BeaconBay/ck](https://github.com/BeaconBay/ck). Gratitude to original authors and contributors.

## License

MIT OR Apache-2.0
