# Canon PIXMA Network Protocol Reference

Reverse-engineered from the Canon PIXMA G3010. These notes cover the two
network protocols Canon uses: CHMP (HTTP-based, used for Wi-Fi scanning)
and BJNP (UDP/TCP-based, used for USB-over-IP).

## CHMP — Canon Home Management Protocol

The G3010 uses CHMP for all Wi-Fi scanner communication. It does **not**
respond to BJNP on ports 8610-8612 over Wi-Fi.

### Transport

HTTP/1.1 on port 80. Every command follows a POST-then-GET pattern:

1. **POST** the command to the endpoint. Printer responds `200 OK` with empty body.
2. **GET** the same endpoint. Printer responds `200 OK` with the actual data (chunked or content-length).

Headers:
```
X-CHMP-Version: 1.4.0
Content-Type: application/octet-stream
Connection: Keep-Alive
```

### Endpoints

The printer exposes multiple endpoints at `/canon/ij/commandN/portM`:

| Endpoint | Purpose |
|---|---|
| `/canon/ij/command1/port1` | Printer control (409 Conflict for scan) |
| `/canon/ij/command2/port1` | Device/print status queries |
| `/canon/ij/command2/port3` | **Scanner** — all scan operations |

### Scan Sequence

Discovered by capturing Canon PRINT iOS app traffic (see `docs/captures/scan_trace.md`):

```
PORT3 (/canon/ij/command2/port3):
  XML VendorCmd ModeShift (jobID=" ")         → OK
  Binary 0xf320 (capability query)            → 24 bytes
  XML StartJob (jobID=00000001, bidi=1)       → OK
  XML VendorCmd ModeShift (jobID=00000001)    → OK
  Binary 0xdb20 StartSession                  → OK
  Binary 0xf320 (capability query)            → 24 bytes
  Binary 0xd820 ScanParam3                    → OK
  Binary 0xd920 ScanStart3                    → OK
  Binary 0xda20 Status3 (poll until ready)    → byte[8]: 0x00→0x02→0x03
  Binary 0xdc20 (get scan dimensions)         → actual line count
  Binary 0xd420 ReadImage (repeat)            → JPEG chunks
  Binary 0xef20 AbortSession                  → OK
  XML EndJob (jobID=00000001)                 → OK
```

### XML Commands

Three XML operations are used for session management:

**StartJob:**
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/">
<ivec:contents><ivec:operation>StartJob</ivec:operation>
<ivec:param_set servicetype="scan">
<ivec:jobID>00000001</ivec:jobID>
<ivec:bidi>1</ivec:bidi>
</ivec:param_set></ivec:contents></cmd>
```

**VendorCmd ModeShift:**
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"
     xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/">
<ivec:contents><ivec:operation>VendorCmd</ivec:operation>
<ivec:param_set servicetype="scan">
<ivec:jobID>00000001</ivec:jobID>
<vcn:ijoperation>ModeShift</vcn:ijoperation>
<vcn:ijmode>1</vcn:ijmode>
</ivec:param_set></ivec:contents></cmd>
```

**EndJob:**
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/">
<ivec:contents><ivec:operation>EndJob</ivec:operation>
<ivec:param_set servicetype="scan">
<ivec:jobID>00000001</ivec:jobID>
</ivec:param_set></ivec:contents></cmd>
```

### Binary Scanner Commands

After ModeShift, the same binary pixma commands used over USB are sent
via HTTP POST/GET. Each command is a 16-byte header, optionally followed
by parameters.

#### Command Header (16 bytes)

| Offset | Size | Field |
|--------|------|-------|
| 0 | 2 | Command code (big-endian) |
| 2 | 2 | Flags (0x0001 for session-active commands) |
| 4 | 10 | Reserved (zeros) |
| 14 | 2 | Param length (includes checksum if >0) |

#### Command Codes

| Code | Name | Params | Description |
|------|------|--------|-------------|
| 0xf320 | CapabilityQuery | 0 (param_len=0x0010) | Query scanner state. Returns 24 bytes. |
| 0xdb20 | StartSession | 0 (flags=0x0001) | Open binary scan session |
| 0xd820 | ScanParam3 | 56 bytes | Set scan parameters |
| 0xd920 | ScanStart3 | 0 (flags=0x0001) | Begin scanning |
| 0xda20 | Status3 | 0 (param_len=0x0008) | Poll scan status. byte[8]: 0=idle, 2=scanning, 3=ready |
| 0xdc20 | GetDimensions | 0 (param_len=0x0008) | Get actual scan line count |
| 0xd420 | ReadImage | 0 (bytes[12-13]=block size in 64KB units) | Read JPEG data block |
| 0xef20 | AbortSession | 0 | Close session |

#### ScanParam3 (0xd820) — 72 bytes total

```
Header: d8 20 00 00 00 00 00 00 00 00 00 00 00 00 00 38
Params (56 bytes):
  [0x00]     01          Source: 1=flatbed
  [0x01]     01
  [0x02]     01
  [0x08-09]  81 2c       X DPI: 300 (0x012c | 0x8000)
  [0x0A-0B]  81 2c       Y DPI: 300
  [0x0C-0F]  00 00 00 00 X offset (pixels)
  [0x10-13]  00 00 00 00 Y offset (pixels)
  [0x14-17]  00 00 09 f6 Width: 2550 pixels (8.5" at 300 DPI)
  [0x18-1B]  00 00 0c e4 Height: 3300 pixels (11" at 300 DPI)
  [0x1C]     08          Color mode: 0x08=color, 0x04=grayscale
  [0x1D]     18          Bits per pixel: 24 (color) or 8 (gray)
  [0x1F]     01
  [0x20]     ff
  [0x21]     82          Output format: 0x82=JPEG, 0x81=raw
  [0x23]     02
  [0x24]     01
  [0x30]     01
  [0x37]     checksum    (all 56 param bytes sum to 0x00)
```

#### ReadImage Response

```
  [0-1]    Status (0x0606=OK)
  [2-7]    Reserved
  [8]      Flags: 0x00=more data, 0x20=last chunk
  [9-11]   Reserved
  [12-15]  Data length (big-endian)
  [16+]    JPEG image data
```

The scanner returns complete JPEG data (starts with `ff d8 ff e0`).
Concatenate all blocks for the full image.

### Status Codes

| Value | Meaning |
|-------|---------|
| 0x0606 | OK |
| 0x1414 | Busy |
| 0x1515 | Failed |

## BJNP — Canon's USB-over-IP Protocol

BJNP wraps USB commands in UDP/TCP packets. The G3010 does **not** respond
to BJNP over Wi-Fi, but it may work over USB. Included here for reference.

### Ports

- 8611: Printing
- 8612: Scanning

### Packet Header (16 bytes, big-endian)

| Offset | Size | Field |
|--------|------|-------|
| 0 | 4 | Magic: `BJNP` (0x42 0x4A 0x4E 0x50) |
| 4 | 1 | Device type: 0x01=print, 0x02=scan. Bit 7=response |
| 5 | 1 | Command code |
| 6 | 2 | Reserved (0) |
| 8 | 2 | Sequence number |
| 10 | 2 | Session ID |
| 12 | 4 | Payload length |

### UDP Commands

| Code | Name | Description |
|------|------|-------------|
| 0x01 | Discover | Broadcast to find printers |
| 0x10 | JobDetails | Send hostname/username to open session |
| 0x11 | Close | Close session |
| 0x30 | GetId | Get IEEE 1284 identity string |

## Discovery

The G3010 is discoverable via mDNS (`_ipp._tcp`). Key TXT records:

```
ty=Canon G3010 series
usb_MDL=G3010 series
Scan=T
UUID=00000000-0000-1000-8000-xxxxxxxxxxxx
```

The scanner endpoint can be found via `_canon-chmp._tcp` with TXT record
`mpath=http://hostname/canon/ij/command1/port1` (note: actual scan endpoint
is `/command2/port3`, not the advertised path).
