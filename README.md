# pixma-rs

A pure Rust driver for Canon PIXMA G-series printers and scanners on macOS. No Canon drivers needed.

This is the first open-source implementation of Canon's CHMP (Canon Home Management Protocol) scanning protocol, reverse-engineered from packet captures.

## What it does

- **Scanning** from Image Capture, Preview, or any macOS app that supports scanners
- **Printing** from any app via IPP Everywhere
- **CLI tools** for scanning and printing from the terminal

## Supported hardware

- Canon PIXMA G3010 (tested)
- Other Canon PIXMA G-series models likely work (same CHMP protocol)

## How it works

Canon's G3010 doesn't support standard scan protocols (eSCL/AirScan) over Wi-Fi. It uses a proprietary HTTP-based protocol called CHMP on port 80.

pixma-rs includes:
- **pixma-bridge**: A daemon that translates between macOS's built-in eSCL (AirScan) scanner protocol and Canon's CHMP protocol. macOS sees it as a native AirScan scanner.
- **pixma**: A CLI tool for scanning and printing directly.

## Install

### Option 1: .pkg installer (easiest)

Download `PixmaDriver-0.1.0.pkg` from [Releases](https://github.com/pdrgds/pixma-rs/releases). Right-click, Open (to bypass Gatekeeper since it's unsigned).

This installs the CLI, starts the bridge daemon, and registers the printer with CUPS. Printing and scanning work immediately.

### Option 2: From source

```bash
git clone https://github.com/pdrgds/pixma-rs.git
cd pixma-rs
cargo build --release

# Start the bridge daemon (scanner appears in Image Capture)
cargo run --release -p pixma-bridge --bin pixma-bridge

# In another terminal, register the printer for printing
cargo run --release -p pixma-cli --bin pixma -- discover
lpadmin -p Canon_G3010 -E -v "ipp://<printer-ip>:631/ipp/print" -m everywhere
```

## CLI usage

```bash
# Find Canon printers on the network
pixma discover

# Scan from the flatbed
pixma scan output.jpg --resolution 300 --color color
pixma scan document.png --resolution 600 --color grayscale

# Print a file
pixma print document.pdf
```

## Architecture

```
macOS Image Capture / Preview
        | (eSCL over HTTP, localhost:8470)
        v
  pixma-bridge daemon
        | (CHMP over HTTP, port 80)
        v
  Canon G3010 (Wi-Fi)
```

The project is a Cargo workspace with three crates:

- **pixma-protocol** — Canon CHMP protocol implementation (discovery, BJNP packets, scan commands, image handling)
- **pixma-cli** — CLI tools (discover, scan, print)
- **pixma-bridge** — eSCL-to-CHMP bridge daemon for macOS system integration

## Protocol documentation

The `docs/` directory contains protocol notes from the reverse-engineering process:

- `docs/bjnp-protocol.md` — BJNP packet format reference
- `docs/captures/scan_trace.md` — Annotated packet capture of Canon PRINT app scanning

## Building the .pkg installer

```bash
./installer/build-pkg.sh
```

Produces `PixmaDriver-0.1.0.pkg`.

## License

MIT OR Apache-2.0
