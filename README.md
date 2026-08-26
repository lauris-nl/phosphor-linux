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

- A Linux desktop capable of running WebKitGTK 4.1 applications
- Node.js 20.19 or newer and npm (build and development only)
- A current stable Rust toolchain (build and development only)
- A current [RRG/Iceman Proxmark3](https://github.com/RfidResearchGroup/proxmark3)
  client that matches the firmware on the reader
- Permission to access the reader's serial device (commonly through the
  distribution's `uucp` or `dialout` group)

### Ubuntu / Debian build dependencies

The following is the current Tauri v2 WebKitGTK 4.1 build set:

```bash
sudo apt update
sudo apt install --no-install-recommends \
  build-essential curl file libayatana-appindicator3-dev librsvg2-dev \
  libssl-dev libwebkit2gtk-4.1-dev libxdo-dev wget
```

### Arch Linux / Manjaro build dependencies

```bash
sudo pacman -S --needed \
  base-devel curl file libappindicator-gtk3 librsvg openssl wget \
  webkit2gtk-4.1 xdotool appmenu-gtk-module
```

These packages compile Phosphor and its native Tauri/WebKitGTK shell. Node.js,
npm, and Rust must also be installed for a source build. See the current
[Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux)
for other distributions.

An installed `.deb` records its GTK/WebKit runtime dependencies for the package
manager. An AppImage bundles most userspace libraries but still relies on the
host kernel, glibc compatibility, graphics/session services, and optionally
FUSE; `APPIMAGE_EXTRACT_AND_RUN=1` can be used where FUSE mounting is disabled.
Neither package format includes the Proxmark3 client.

### RRG client and reader access

The RRG/Iceman `proxmark3` executable is a separate runtime dependency. Confirm
that it matches the reader firmware and works independently:

```bash
proxmark3 --version
proxmark3 -p /dev/serial/by-id/your-proxmark-device -f -c "hw version"
```

If serial access is denied, add the user to the distribution's serial-device
group (`uucp` on Arch/Manjaro or commonly `dialout` on Debian/Ubuntu), then log
out and back in. Check the actual device group with `ls -l /dev/ttyACM0` rather
than assuming a group name.

## Build and run

```bash
git clone https://github.com/lauris-nl/phosphor-linux.git
cd phosphor-linux
npm ci
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml
./scripts/phosphor
```

The launcher prefers a unique Proxmark entry under `/dev/serial/by-id/`, falls
back to a unique existing ACM/USB serial node, and finds `proxmark3` through
`PATH`.

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

GitHub Actions performs these checks on a fresh Ubuntu 24.04 runner without an
RFID reader, serial device, firmware image, or PM3 client binary. The launcher
tests use temporary fixtures for missing-client and missing-reader errors.

## Linux packages

Tauri is configured for AppImage and Debian package output:

```bash
npm run tauri build -- --bundles appimage
npm run tauri build -- --bundles deb
```

Minimal Debian/Ubuntu packaging environments also need `patchelf` and
`xdg-utils` for the AppImage bundler. Install `libfuse2` to mount the resulting
AppImage through FUSE, or launch it with `APPIMAGE_EXTRACT_AND_RUN=1` on hosts
where FUSE mounting is unavailable.

Generated packages are placed under `src-tauri/target/release/bundle/` and are
ignored by Git. AppImages should be built on the oldest supported Linux base;
Ubuntu 22.04 or Debian 12 are suitable WebKitGTK 4.1 baselines according to the
Tauri documentation. Building on a newer system can require a newer glibc.

The AppImage bundles Phosphor and its media framework, but deliberately does not
bundle `proxmark3`. Bundling RRG later would require a reviewed update policy and
GPL source/distribution plan. Until then, users install a matching current RRG
client separately or set `PHOSPHOR_MODERN_PM3_BIN`.

No Linux package or firmware image is currently published as a GitHub Release.

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
