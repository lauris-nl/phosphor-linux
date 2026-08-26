# Phosphor Linux

Native Linux support for the [Phosphor](https://github.com/nikitaart2000/phosphor)
Proxmark3 GUI.

This project preserves Phosphor's upstream history and GPL-3.0 license while
adding Linux USB-CDC discovery, current RRG/Iceman client integration, generic
reader support, and fixes for reliable saved-tag and T55x7 write workflows.
Original Phosphor was created by **nik shuv**; the Linux work is maintained
separately by the Phosphor Linux project.

> Use RFID read/write functionality only on tags and systems you own or are
> authorized to test.

## Verified Linux features

- Native Tauri application on Linux (no Wine)
- Current RRG/Iceman Proxmark3 client command syntax
- USB-CDC discovery through `/dev/serial/by-id/`, `/dev/ttyACM*`, and
  `/dev/ttyUSB*`
- Optional explicit reader and client paths
- PM3GENERIC hardware/version reporting without unsafe automatic clone flashing
- LF and HF reader connectivity
- T55x7 detection, wipe, write, and read-back verification
- Actionable command, timeout, busy-port, incompatible-tag, and USB-disconnect
  errors
- Saved-card loading that remains separate from the physical `WRITE` action
- Quiet production startup and an explicit development launcher

The Linux flow has been physically tested with a PM3GENERIC-compatible reader
using an AT91SAM7S512 with 512 KB flash and matching current RRG firmware/client.
That observation does **not** identify every generic or inexpensive clone:
FPGA, flash, antenna, LED, and board wiring can differ.

## Requirements

- A Linux desktop with WebKitGTK/Tauri v2 build prerequisites
- Node.js and npm
- A current Rust toolchain
- A current [RRG/Iceman Proxmark3](https://github.com/RfidResearchGroup/proxmark3)
  client that matches the firmware on the reader
- Permission to access the reader's serial device (commonly through the
  distribution's `uucp` or `dialout` group)

Install the platform packages listed in the
[Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux),
then confirm that the PM3 client works independently:

```bash
proxmark3 --version
proxmark3 -p /dev/serial/by-id/your-proxmark-device -f -c "hw version"
```

## Build and run

```bash
git clone https://github.com/lauris-nl/phosphor-linux.git
cd phosphor-linux
npm ci
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml
./scripts/phosphor
```

The launcher prefers a unique Proxmark entry under `/dev/serial/by-id/`, falls back to a
unique existing ACM/USB serial node, and finds `proxmark3` through `PATH`.

Override either path when discovery is ambiguous or the client is installed in
a non-standard location:

```bash
PHOSPHOR_PM3_PORT=/dev/serial/by-id/your-proxmark-device \
PHOSPHOR_MODERN_PM3_BIN=/opt/proxmark3/bin/proxmark3 \
./scripts/phosphor
```

The configured PM3 executable must be the current RRG client matching the
reader firmware. Legacy pre-RRG clients and firmware do not implement the
modern command/protocol surface used by Phosphor.

## Development

Set the stable reader path explicitly for development:

```bash
PHOSPHOR_PM3_PORT=/dev/serial/by-id/your-proxmark-device \
PHOSPHOR_MODERN_PM3_BIN=/path/to/proxmark3 \
RUST_LOG=debug RUST_BACKTRACE=1 \
./scripts/phosphor-dev
```

Run the validation suite with:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
git diff --check
```

`src-tauri/binaries/`, build output, PM3 logs, dumps, and saved-tag data are
intentionally excluded from Git. This repository does not vendor the RRG source
tree, a locally compiled PM3 client, firmware images, or private RFID data.

## Firmware safety

Ordinary Phosphor installation does not require flashing firmware when the
reader already runs firmware compatible with the installed RRG client.

Automatic firmware flashing is disabled for generic clones. Do not select an
image based only on the words “generic,” “Easy,” MCU capacity, or a seller's
listing. Hardware target identification, bootloader compatibility, recovery,
and firmware migration must be reviewed separately for the exact board. This
project does not publish or recommend a universal generic-clone firmware image.

## Known limits

- Hardware support is bounded by the commands implemented by the installed RRG
  client and matching reader firmware.
- DESFire is detection-only in the inherited workflow.
- Generic-clone automatic firmware updates are intentionally unavailable.
- Multiple connected serial readers require `PHOSPHOR_PM3_PORT`.

## Upstream and dependencies

- Original application: [nikitaart2000/phosphor](https://github.com/nikitaart2000/phosphor)
- PM3 client/firmware: [RfidResearchGroup/proxmark3](https://github.com/RfidResearchGroup/proxmark3), a separate project and dependency
- Linux project contact: [phosphor-linux@lauris.nl](mailto:phosphor-linux@lauris.nl)

## License

[GPL-3.0](LICENSE). Original copyright and attribution are retained from
upstream; subsequent Linux extensions are distributed under the same license.
