# Canon PRINT App Scan Protocol Trace

Captured from Canon PRINT iOS app (iPhone 192.168.0.3) scanning on a Canon
G3010 printer (192.168.0.9) over CHMP (Canon Home Management Protocol).

**Source**: `canon_scan.pcapng` captured 2026-03-31.

**Summary**: The scan produces a **JPEG** image at **300 DPI, color, US Letter
(8.5" x 11")**, totalling ~376 KB of JPEG data across 21 chunks.

---

## Transport: CHMP over HTTP

Every command follows a **POST then GET** pattern on port 80:

1. **POST** the command (XML or binary pixma bytes) to the endpoint.
   Printer responds `200 OK, Content-Length: 0` (acknowledgement only).
2. **GET** the same endpoint to retrieve the actual response.
   Printer responds `200 OK` with either `Transfer-Encoding: chunked`
   (for XML) or `Content-Length: N` (for binary).

Both requests and responses use `Content-Type: application/octet-stream`.

### HTTP Headers (Canon PRINT app)

POST headers:
```
POST /canon/ij/command2/port3 HTTP/1.1
Host: 192.168.0.9
X-CHMP-Version: 1.4.0
X-CHMP-Timeout: 20
Connection: Keep-Alive
Content-Length: <n>
Content-Type: application/octet-stream
```

GET headers:
```
GET /canon/ij/command2/port3 HTTP/1.1
Host: 192.168.0.9
Connection: Keep-Alive
Content-Type: application/octet-stream
X-CHMP-Version: 1.4.0
```

Note: The app sends `X-CHMP-Version: 1.4.0`. The printer always responds
with `X-CHMP-Version: 1.1.0`.

---

## Complete Request Sequence

### Phase 1: Device Status Polling (port1)

Six POST/GET exchanges on `/canon/ij/command2/port1` before the scan begins.

#### Request 1 (frame 4, t=0.01s) -- GetStatus device

```xml
<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"><ivec:contents><ivec:operation>GetStatus</ivec:operation><ivec:param_set servicetype="device"></ivec:param_set></ivec:contents></cmd>
```

Response (frame 28, chunked, 1319 bytes):
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"
xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/">
<ivec:contents>
<ivec:operation>GetStatusResponse</ivec:operation>
<ivec:param_set servicetype="device">
<ivec:response>OK</ivec:response>
<ivec:response_detail/>
<vcn:eid>
<vcn:url_id>00</vcn:url_id>
<vcn:redirect_url><![CDATA[http://rs.ciggws.net/rd.cgi?FNC=RUI_EID2&RES=9&DEV=G3010+series&CNM_SEP=0&mdl=G3010+series&low=0&out=0&ac=0&srcmdl=6&resid=Other&hriid=1]]></vcn:redirect_url>
</vcn:eid>
<ivec:network_interface>
<ivec:interface_set id="wireless0">
<vcn:communication_method>
<vcn:item>infrastructure_mode</vcn:item>
</vcn:communication_method>
<ivec:ip>
<ivec:hwaddress><![CDATA[6C:3C:7C:9D:B7:F8]]></ivec:hwaddress>
</ivec:ip>
</ivec:interface_set>
<ivec:interface_set id="wireless1">
<vcn:communication_method>
<vcn:item>access_point_mode</vcn:item>
</vcn:communication_method>
<ivec:ip>
<ivec:hwaddress><![CDATA[6E:3C:7C:9D:B7:F8]]></ivec:hwaddress>
</ivec:ip>
</ivec:interface_set>
</ivec:network_interface>
</ivec:param_set>
</ivec:contents>
</cmd>
```

#### Requests 2-6 (frames 33, 60, 85, 104, 129; t=2.2s-19.5s) -- GetStatus print x5

```xml
<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"><ivec:contents><ivec:operation>GetStatus</ivec:operation><ivec:param_set servicetype="print"></ivec:param_set></ivec:contents></cmd>
```

All five responses are identical (chunked, 1101 bytes each):
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"
xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/">
<ivec:contents>
<ivec:operation>GetStatusResponse</ivec:operation>
<ivec:param_set servicetype="print">
<ivec:response>OK</ivec:response>
<ivec:response_detail></ivec:response_detail>
<ivec:status>idle</ivec:status>
<ivec:status_detail/>
<ivec:current_support_code/>
<vcn:eid>
<vcn:url_id>00</vcn:url_id>
</vcn:eid>
<ivec:status_detail_list/>
<vcn:hri>1</vcn:hri>
<vcn:pdr>9</vcn:pdr>
<vcn:hrc>00</vcn:hrc>
<vcn:msi>
<vcn:item type="A">OFF</vcn:item>
<vcn:item type="B">240</vcn:item>
<vcn:item type="D">auto</vcn:item>
<vcn:item type="E">3</vcn:item>
<vcn:item type="J">OFF</vcn:item>
<vcn:item type="K">21000700</vcn:item>
<vcn:item type="M">ON</vcn:item>
</vcn:msi>
<vcn:rrc>0</vcn:rrc>
<ivec:jobinfo/>
</ivec:param_set>
</ivec:contents>
</cmd>
```

---

### Phase 2: Scan Session Setup (port3, XML)

All remaining requests target `/canon/ij/command2/port3`.

#### Request 7 (frame 154, t=21.06s) -- VendorCmd ModeShift (pre-session)

Note: `jobID` contains a single space -- this is BEFORE StartJob.

```xml
<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/" xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/"><ivec:contents><ivec:operation>VendorCmd</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID> </ivec:jobID><vcn:ijoperation>ModeShift</vcn:ijoperation><vcn:ijmode>1</vcn:ijmode></ivec:param_set></ivec:contents></cmd>
```

Response (frame 183, chunked):
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"
xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/">
<ivec:contents>
<ivec:operation>VendorCmdResponse</ivec:operation>
<ivec:param_set servicetype="scan">
<ivec:response>OK</ivec:response>
<ivec:response_detail/>
<vcn:ijoperation>ModeShiftResponse</vcn:ijoperation>
<vcn:ijresponse>OK</vcn:ijresponse>
<vcn:ijresponse_detail/>
</ivec:param_set>
</ivec:contents>
</cmd>
```

#### Request 8 (frame 185, t=21.84s) -- Binary 0xf320 (unknown/capability query)

```
POST body (16 bytes):
f3 20 00 00 00 00 00 00 00 00 00 00 00 00 00 10
```

Response (24 bytes):
```
06 06 00 00 00 00 00 00 01 00 00 01 00 03 00 00
00 00 00 00 00 00 00 fb
```

Interpretation:
- `0606` = status OK
- byte[8] = 0x01
- bytes[9-11] = 0x000001
- bytes[12-13] = 0x0003
- Last byte `0xfb` is a checksum (all bytes sum to 0x0c mod 256)

#### Request 9 (frame 227, t=22.55s) -- StartJob

```xml
<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"><ivec:contents><ivec:operation>StartJob</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID>00000001</ivec:jobID><ivec:bidi>1</ivec:bidi></ivec:param_set></ivec:contents></cmd>
```

Response (frame 251, chunked):
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/">
<ivec:contents>
<ivec:operation>StartJobResponse</ivec:operation>
<ivec:param_set servicetype="scan">
<ivec:response>OK</ivec:response>
<ivec:response_detail/>
<ivec:jobID>00000001</ivec:jobID>
</ivec:param_set>
</ivec:contents>
</cmd>
```

#### Request 10 (frame 253, t=23.14s) -- VendorCmd ModeShift (post-session)

Same XML as request 7 but with `jobID` = `00000001`:

```xml
<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/" xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/"><ivec:contents><ivec:operation>VendorCmd</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID>00000001</ivec:jobID><vcn:ijoperation>ModeShift</vcn:ijoperation><vcn:ijmode>1</vcn:ijmode></ivec:param_set></ivec:contents></cmd>
```

Response: same VendorCmdResponse OK as request 7.

---

### Phase 3: Binary Pixma Scan Setup (port3)

#### Request 11 (frame 278, t=23.41s) -- 0xdb20 StartSession

```
POST body (16 bytes):
db 20 00 01 00 00 00 00 00 00 00 00 00 00 00 00
```

Note: byte[2-3] = 0x0001 (session flag).

Response (8 bytes):
```
06 06 00 00 00 00 00 00
```

Status OK, minimal ack.

#### Request 12 (frame 303, t=23.69s) -- 0xf320 (capability/info query, 2nd time)

```
POST body (16 bytes):
f3 20 00 00 00 00 00 00 00 00 00 00 00 00 00 10
```

Response (24 bytes):
```
06 06 00 00 00 00 00 00 01 00 00 01 00 03 00 00
00 00 21 00 00 00 00 da
```

Compared to the first f320 response, byte[18] changed from `0x00` to `0x21`
(scanner now in scan-ready mode after session start). Checksum `0xda` validates.

#### Request 13 (frame 328, t=23.96s) -- 0xd820 ScanParam3

```
POST body (72 bytes = 16 header + 56 params):
d8 20 00 00 00 00 00 00 00 00 00 00 00 00 00 38
01 01 01 00 00 00 00 00 81 2c 81 2c 00 00 00 00
00 00 00 00 00 00 09 f6 00 00 0c e4 08 18 00 01
ff 82 00 02 01 00 00 00 00 00 00 00 00 01 00 00
01 00 00 00 00 00 00 0d
```

Decoded parameters (56 bytes at offset 16):

| Offset | Hex | Value | Field |
|--------|-----|-------|-------|
| 0x00 | `01` | 1 | Source: flatbed |
| 0x01 | `01` | 1 | (unknown) |
| 0x02 | `01` | 1 | (unknown) |
| 0x03-07 | `00...` | 0 | (padding) |
| 0x08-09 | `81 2c` | 300 | X DPI (0x012c = 300, flag 0x8000 set) |
| 0x0A-0B | `81 2c` | 300 | Y DPI (0x012c = 300, flag 0x8000 set) |
| 0x0C-0F | `00 00 00 00` | 0 | X offset (pixels) |
| 0x10-13 | `00 00 00 00` | 0 | Y offset (pixels) |
| 0x14-17 | `00 00 09 f6` | 2550 | Width (pixels) = 8.50" at 300 DPI |
| 0x18-1B | `00 00 0c e4` | 3300 | Height (pixels) = 11.00" at 300 DPI |
| 0x1C | `08` | 8 | Color mode (0x08=color, 0x04=grayscale) |
| 0x1D | `18` | 24 | Bits per pixel (24=RGB, 8=gray) |
| 0x1E | `00` | 0 | (padding) |
| 0x1F | `01` | 1 | (flag) |
| 0x20 | `ff` | 255 | (JPEG quality?) |
| 0x21 | `82` | 130 | **0x82** (Canon app uses 0x82; SANE uses 0x81) |
| 0x22 | `00` | 0 | (padding) |
| 0x23 | `02` | 2 | (flag) |
| 0x24 | `01` | 1 | (flag) |
| 0x25-2F | `00...` | 0 | (padding) |
| 0x30 | `01` | 1 | (flag) |
| 0x31-36 | `00...` | 0 | (padding) |
| 0x37 | `0d` | 13 | **Checksum** (all 56 param bytes sum to 0x00) |

**Key difference vs our driver**: byte 0x21 = `0x82` in the Canon app, vs `0x81`
in the SANE-derived code. This may control the output format -- the Canon app
receives **JPEG** data, while SANE typically receives raw pixels.

**Scan area**: 2550 x 3300 pixels = US Letter (8.5" x 11.0") at 300 DPI.

Response (8 bytes): `06 06 00 00 00 00 00 00` -- Status OK.

#### Request 14 (frame 354, t=24.24s) -- 0xd920 ScanStart3

```
POST body (16 bytes):
d9 20 00 01 00 00 00 00 00 00 00 00 00 00 00 00
```

Note: byte[2-3] = 0x0001 (session active).

Response (8 bytes): `06 06 00 00 00 00 00 00` -- Status OK.

---

### Phase 4: Calibration/Scanning Polling (port3, 0xda20)

19 rounds of 0xda20 Status3 polling, approximately every 250ms.

```
POST body (16 bytes, same every time):
da 20 00 00 00 00 00 00 00 00 00 00 00 00 00 08
```

Note: bytes[14-15] = 0x0008 (expected response size hint).

Response (16 bytes each):

| Frames | byte[8] | Meaning | Count |
|--------|---------|---------|-------|
| 381-508 | `0x00` | Idle / initializing | 6 |
| 532-808 | `0x02` | Scanning / calibrating (lamp warm-up) | 12 |
| 835 | `0x03` | **Scan data ready** | 1 |

Status byte values:
- `0x00` = idle (no data, no activity)
- `0x02` = scanning in progress (lamp/head moving)
- `0x03` = data ready to read

Full 16-byte responses:
```
idle:       06 06 00 00 00 00 00 00  00 00 00 00 00 00 00 00
scanning:   06 06 00 00 00 00 00 00  02 00 00 00 00 00 00 fe
ready:      06 06 00 00 00 00 00 00  03 00 00 00 00 00 00 fd
```

Last byte is a checksum (all 16 bytes sum to 0x0c mod 256).

---

### Phase 5: Get Actual Scan Dimensions (port3, 0xdc20)

#### Request 34 (frame 837, t=29.87s) -- 0xdc20 (get scan info)

Sent immediately after `da20` returns `byte[8]=0x03` (ready).

```
POST body (16 bytes):
dc 20 00 00 00 00 00 00 00 00 00 00 00 00 00 08
```

Response (16 bytes):
```
06 06 00 00 00 00 00 00 00 00 0d 99 00 00 00 5a
```

Decoded:
- Status: `0x0606` = OK
- byte[8]: `0x00`
- bytes[10-11]: `0x0d99` = **3481** = actual number of scan lines
- byte[15]: `0x5a` = checksum

The scanner delivers **3481 lines** even though 3300 were requested
(181 extra lines = 0.60" overshoot at 300 DPI). Software is expected
to crop to the requested dimensions.

---

### Phase 6: Image Data Transfer (port3, 0xd420)

Repeated 0xd420 ReadImage commands fetch the JPEG data.

```
POST body (16 bytes, same every time):
d4 20 00 00 00 00 00 00 00 00 00 00 00 20 00 00
```

Note: bytes[12-13] = `0x0020` = 32 (block size in 64KB units = **2 MB max**).
bytes[14-15] = `0x0000` (no param payload appended).

**This differs from SANE-style drivers** which put the block size request in an
8-byte param area after the header (total 24 bytes). The Canon PRINT app uses
the header-only 16-byte format.

#### Response format

Each response is a binary block:

```
Offset  Size  Field
0       2     Status (0x0606 = OK)
2       6     Reserved (zeros)
8       1     Flags: 0x00 = more data
                     0x20 = last data chunk
                     0x28 = session closed (after ef20)
9       3     Reserved
12      4     Data length (big-endian, bytes of image data following header)
16      N     Image data (JPEG bytes)
```

#### Transfer timeline

| Frame | Time (s) | Data Length | Notes |
|-------|----------|-------------|-------|
| 861-938 | 29.9-32.8 | 0 | Empty responses (printer buffering) |
| 1001 | 33.0 | 32,768 | First data chunk, starts with `ff d8 ff e0` (JPEG SOI) |
| 1081 | 33.1 | 49,152 | Large chunk |
| 1178-2361 | 34.0-43.2 | 16,384 each | 18 regular chunks |
| 2398 | 43.5 | 8,319 | **Last chunk** -- byte[8]=`0x20` (end-of-data flag) |

**Total JPEG payload**: 32,768 + 49,152 + (18 x 16,384) + 8,319 = **385,151 bytes** (376 KB)

The output is a complete JPEG file (SOI marker `ff d8` at start).

#### End-of-scan detection

The last data chunk has `byte[8] = 0x20`. This is the end-of-data flag.

**Important**: Our driver checks `byte[8] & 0x28 == 0x28` which requires BOTH
bits 5 and 3. The Canon app only sets bit 5 (`0x20`) in the last data chunk.
The `0x28` pattern only appears in the `ef20` AbortSession response (after
all data has been read). The correct end-of-data check should be `byte[8] & 0x20 != 0`.

---

### Phase 7: Session Teardown (port3)

#### Request 115 (frame 2427, t=43.87s) -- 0xef20 AbortSession

```
POST body (16 bytes):
ef 20 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

Response (16 bytes):
```
06 06 00 00 00 00 00 00 28 00 00 00 00 00 00 00
```

byte[8] = `0x28` = session terminated. This is the ONLY response with `0x28`.

#### Request 116 (frame 2453, t=44.15s) -- EndJob

```xml
<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"><ivec:contents><ivec:operation>EndJob</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID>00000001</ivec:jobID></ivec:param_set></ivec:contents></cmd>
```

Response (frame 2479, chunked):
```xml
<?xml version="1.0" encoding="utf-8" ?>
<cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/">
<ivec:contents>
<ivec:operation>EndJobResponse</ivec:operation>
<ivec:param_set servicetype="scan">
<ivec:response>OK</ivec:response>
<ivec:response_detail/>
<ivec:jobID>00000001</ivec:jobID>
</ivec:param_set>
</ivec:contents>
</cmd>
```

---

## Summary: Canonical Scan Sequence

```
PORT1 (status):
  POST GetStatus servicetype="device"  -> device info
  POST GetStatus servicetype="print"   -> printer idle (x5)

PORT3 (scan):
  POST XML VendorCmd ModeShift (jobID=" ")     -> OK
  POST 0xf320 (capability query)               -> 24-byte info
  POST XML StartJob (jobID=00000001, bidi=1)   -> OK
  POST XML VendorCmd ModeShift (jobID=00000001)-> OK
  POST 0xdb20 StartSession                     -> OK
  POST 0xf320 (capability query, 2nd)          -> 24-byte info (0x21 flag)
  POST 0xd820 ScanParam3 (300dpi, color, Letter, 0x82)  -> OK
  POST 0xd920 ScanStart3                       -> OK
  POST 0xda20 Status3 (poll x19)               -> 0x00..0x02..0x03
  POST 0xdc20 (get dimensions)                 -> actual_lines=3481
  POST 0xd420 ReadImage (x~80)                 -> JPEG chunks (376 KB)
  POST 0xef20 AbortSession                     -> 0x28 (closed)
  POST XML EndJob (jobID=00000001)             -> OK
```

Total time: ~44 seconds (21s pre-scan status polling + 23s scan session).

---

## Key Findings for Our Driver

1. **JPEG output mode**: The Canon app sets byte `0x21` of `ScanParam3` to
   `0x82` and receives JPEG output. Our code uses `0x81` (SANE default) which
   likely requests raw pixel data.

2. **End-of-scan flag**: The last data chunk has `byte[8] = 0x20`, NOT `0x28`.
   Our check `byte[8] & 0x28 == 0x28` will never trigger from data alone. Fix:
   change to `byte[8] & 0x20 != 0` or read until data_len == 0 with the flag.

3. **ReadImage command format**: The Canon app uses a 16-byte header-only
   command with the block size at bytes[12-13] (in 64KB units). Our driver
   appends 8 bytes of params. Both should work, but matching the Canon app
   format is safer.

4. **Pre-session ModeShift**: The Canon app sends ModeShift with a blank jobID
   BEFORE StartJob. Our driver only sends it after StartJob.

5. **0xf320 and 0xdc20**: Two binary commands not in our driver:
   - `0xf320` queries scanner capability/state (sent before and after session start)
   - `0xdc20` retrieves actual scan dimensions after `da20` reports ready (byte[8]=3)

6. **0xdb20 StartSession byte[2-3]**: Canon app sends `0x0001` (not `0x0000`).
   Our code sends zeros. The `0x0001` likely signals "session with data transfer".

7. **0xd920 ScanStart3 byte[2-3]**: Canon app sends `0x0001`. Our code sends zeros.

8. **Scan dimensions**: Width 2550 is NOT 32-byte aligned (2550 % 32 = 22).
   The Canon app does not align width to 32 pixels for JPEG mode.
