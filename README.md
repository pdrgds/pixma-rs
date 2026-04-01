# Canon PIXMA G3010 Driver for macOS

**Print and scan with your Canon PIXMA G3010 on macOS — no Canon software needed.**

Canon doesn't provide a macOS driver for the PIXMA G3010 (or most G-series printers). This project fixes that. It's an open-source driver that gives you full printing and scanning support on macOS Ventura, Sonoma, and Sequoia.

Built in Rust. First open-source implementation of Canon's CHMP scanning protocol, reverse-engineered from network captures.

## Features

- **Scanning** — works with Image Capture, Preview, and any macOS app that supports scanners
- **Printing** — works from any app via IPP Everywhere (AirPrint-compatible)
- **CLI** — scan and print from the terminal
- **No Canon software** — fully standalone, no IJ Scan Utility or Canon drivers required
- **Wi-Fi** — connects to your printer over the network

## Supported printers

| Printer | Status |
|---------|--------|
| Canon PIXMA G3010 | Tested and working |
| Canon PIXMA G2010, G4010, G3020, G2020 | Should work (same CHMP protocol) |
| Other Canon PIXMA G-series | Likely compatible |

## Why this exists

Canon's PIXMA G3010 only supports scanning via a proprietary protocol (CHMP) over Wi-Fi — it doesn't speak eSCL/AirScan, so macOS can't see it as a scanner. Canon's own IJ Scan Utility hasn't been updated for recent macOS versions and doesn't support Apple Silicon natively.

This driver bridges the gap: it translates between macOS's native AirScan protocol and Canon's CHMP, so your G3010 scanner appears as a native device in Image Capture and Preview. Printing works out of the box via IPP Everywhere.

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

## Legal

This project was developed through clean-room reverse engineering of network
traffic from a personally owned device, for the purpose of interoperability.
No Canon proprietary software was decompiled or redistributed.

## License

MIT OR Apache-2.0
