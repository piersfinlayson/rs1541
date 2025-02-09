# rs1541
Rust bindings and helper functions for accessing Commodore disk drives through OpenCBM.

## Overview
This crate provides idiomatic Rust interfaces to OpenCBM, allowing easy control of Commodore disk drives (like the 1541) using modern USB devices such as the XUM1541.
Thread-safe access to OpenCBM through protected mutex handles enables safe usage in multi-threaded and async applications.

## Features
* Safe Rust wrappers around OpenCBM's C interface
* RAII-based driver management - no manual open/close needed
* Thread-safe access for multi-threaded and async applications
* Ergonomic error handling using Rust's Result type
* Directory parsing with structured data types
* Strong typing for CBM-specific concepts (error codes, status messages, etc.)

## Pre-requisites
* [Rust](https://www.rust-lang.org/tools/install)
* [OpenCBM](https://github.com/piersfinlayson/OpenCBM) installed and configured
* clang (used to generate OpenCBM bindings)
* XUM1541 (or compatible) USB device
* Appropriate system permissions for USB access

See [Pre-requisities - More Detail](#pre-requisites---more-detail) to help with these.

## Quick Start

### Create a new rust project
```bash
cargo new my-1541-app
cd my-1541-app
```

### Add the rs1541 dependency
```toml
[dependencies]
rs1541 = "0.1"
```

Replace the contents of main.rs with:
```rust
use rs1541::{Cbm, Error};

fn main() -> Result<(), Error> {
    // Driver automatically opens on creation and closes on drop
    let cbm = Cbm::new(None, None)?;

    // Get drive information
    let id = cbm.identify(8)?;
    println!("Drive type at device 8: {}", id);

    // Check drive status
    let status = cbm.get_status(8)?;
    println!("Drive status: {}", status);

    // Read directory
    let dir = cbm.dir(8, None)?;
    println!("Directory listing:\n{}", dir);

    Ok(())
}
```

Then:
```bash
cargo build
cargo run
```

If you have a 1541 disk drive attached via an XUM1541, with the 1541 demo disk installed you should see:

```markdown
<details>
<summary>Example Output (with 1541 demo disk)</summary>
Drive type at device 8: CBM 1541: 1540 or 1541
Drive status: 00,OK,00,00
Directory listing:
Drive 0 Header: "test/demo  1/85" ID: 84
Filename: "how to use.prg"           Blocks:  14
Filename: "how part two.prg"         Blocks:   8
Filename: "how part three.prg"       Blocks:   7
Filename: "vic-20 wedge.prg"         Blocks:   4
Filename: "c-64 wedge.prg"           Blocks:   1
Filename: "dos 5.1.prg"              Blocks:   4
Filename: "printer test.prg"         Blocks:   9
Filename: "disk addr change.prg"     Blocks:   6
Filename: "view bam.prg"             Blocks:  12
Filename: "display t&s.prg"          Blocks:  15
Filename: "check disk.prg"           Blocks:   4
Filename: "performance test.prg"     Blocks:  11
Filename: "seq.file.demo.prg"        Blocks:   5
Filename: "rel.file.demo.prg"        Blocks:  18
Filename: "sd.backup.c16.prg"        Blocks:   7
Filename: "sd.backup.plus4.prg"      Blocks:   7
Filename: "sd.backup.c64.prg"        Blocks:  10
Filename: "print.64.util.prg"        Blocks:   7
Filename: "print.c16.util.prg"       Blocks:   7
Filename: "print.+4.util.prg"        Blocks:   7
Filename: "uni-copy.prg"             Blocks:  13
Filename: "c64 basic demo.prg"       Blocks:  30
Filename: "+4 basic demo.prg"        Blocks:  35
Filename: "load address.prg"         Blocks:   8
Filename: "unscratch.prg"            Blocks:   7
Filename: "header change.prg"        Blocks:   5
Free blocks: 403
</details>
```

## Examples

See ```examples/```.

  To run examples:

```bash
cargo run --example basic
cargo run --example async
```

### Basic Example

The directory listing example shown above.

### Async Example

This example demonstrates thread-safe concurrent access to a Commodore disk drive using rs1541's mutex-protected OpenCBM handle. It spawns two concurrent tasks:
* Task 1 identifies the drive and reads its directory
* Task 2 polls the drive status multiple times at fixed intervals

### Format Example

Formatting a disk

### CLI

Provides an interactive CLI to exercise some of the rs1541/OpenCBM functionality

## Pre-requisites - More Detail

### OpenCBM

rs1541 relies on OpenCBM.  You must build and install OpenCBM before building rs1541.  I've made a few mods to OpenCBM to make it work more reliably so I suggest you use my fork.  You can build and isntall it like this:

```
sudo apt-get install build-essential libusb-1.0-0-dev usbutils cc65 linux-headers-$(uname -r)
git clone https://github.com/piersfinlayson/OpenCBM
cd OpemCBM
git checkout all-features
make -f LINUX/Makefile plugin
sudo make -f LINUX/Makefile install install-plugin
sudo adduser $USER opencbm
```

To see if you can access your XUM1541 USB device make sure it's plugged in then:

```
cbmctrl detect
```

This should return nothing if you have no drives connected, otherwise a list of detect drives.

See [Troubleshooting](#troubleshooting) below if you get errors when running cbmctrl - permission issues are common.

### clang

Add clang-dev so that rs1541 can correctly generate the Rust bindings to the OpenCBM library C functions:
```
sudo apt install build-essential llvm-dev libclang-dev clang
```

### Rust

If you don't already have Rust installed:
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Troubleshooting

### Logging

Use ```RUST_LOG=<log level>``` before the 1541fs command.  If you're hitting problems then ```RUST_LOG=debug``` is a good bet.  If 1541fs starts 1541fsd (i.e. it wasn't already running), this log level (via this environment variable) will also be propogated to the invoked 1541fsd.

1541fs logs go to stdout.

1541fsd logs to syslog, so to /var/log/syslog or wherever syslog/rsyslog is configured to output.  You can put this in your ```/etc/rsyslog.conf``` if you'd like 1541fsd logs to go to ```/var/log/1541fs.log```:

```
$template CustomFormat,"%TIMESTAMP% %HOSTNAME% %syslogtag% %syslogseverity-text:::uppercase% %msg%\n"
:programname, startswith, "1541fs" -/var/log/1541fs.log;CustomFormat
```

### XUM1541/USB device Permission Issues

If you don't have XUM1541 USB permissions correct on your system you'll probably get something like this:

```
error: cannot query product name: error sending control message: Operation not permitted
error: no xum1541 device found
error: cannot query product name: error sending control message: Operation not permitted
error: no xum1541 device found
cbmctrl: libusb/xum1541:: Operation not permitted
```

It can be a bit of a pain getting permissions right for accessing the XUM1541 USB device, unless you want to run everything as sudo (not recommended for security reasons).

If you get any kind of ```PermissionDenied``` or ```Operation not permitted``` errors I recommend replacing /etc/udev/rules.d/45-opencbm-xum1541.rules with this content:

```
SUBSYSTEM!="usb_device", ACTION!="add", GOTO="opencbm_rules_end"

# xum1541
SUBSYSTEM=="usb", ATTRS{idVendor}=="16d0", ATTRS{idProduct}=="0504", MODE="0666", GROUP="plugdev", TAG+="uaccess"

# xum1541 in DFU mode
SUBSYSTEM=="usb", ATTRS{idVendor}=="03eb", ATTRS{idProduct}=="2ff0", MODE="0666", GROUP="plugdev", TAG+="uaccess"

LABEL="opencbm_rules_end"
```

Then add your user to the plugdev group:

```
sudo usermod -a -G plugdev $USER
```

You may need to restart your shell at this point.

Then reload udev rules:

```
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Then reattach your USB device (XUM1541) and try ```cbmctrl detect" again.  Until this works you're unlikely to be get rs1541 working.

### WSL

You can use WSL (WSL2 to be precise) run rs1541.  You must use usbipd in order to connect your XUM1541 USB device to the WSL kernel.  I've found that this stops working after a while, and the wsl instance must be shutdown and restarted in order to get it working again.

### XUM1541

To see logs from XUM1541 add this to the front of the command you run the daemon with:

```
XUM1541_DEBUG=10
```

The XUM1541 sometimes gets into a bad state.  You can kill 1541fsd and then run ```usbreset``` to reset the device.  Run it without arguments to see what deivce number you need.  For example:

```
usbreset 001/011
```

### libusb1.0

While OpenCBM and the XUM1541 code supports both libusb0.1 and 1.0 I strongly recommend you use 1.0 - the apt install command earlier in this file installs the correct libusb1.0 packages.

With libusb0.1 I've seen odd segmentation faults and other issues.

To verify you really are using libusb1.0 run this after installing OpenCBM and the XUM1541 plugin):

```
ldd /usr/local/lib/opencbm/plugin/libopencbm-xum1541.so
```

You should see something like this:

```
linux-vdso.so.1 (0x00007ffc21171000)
        libusb-1.0.so.0 => /lib/x86_64-linux-gnu/libusb-1.0.so.0 (0x00007fa4566ac000)
        libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007fa456483000)
        libudev.so.1 => /lib/x86_64-linux-gnu/libudev.so.1 (0x00007fa456459000)
        /lib64/ld-linux-x86-64.so.2 (0x00007fa4566df000)
```

In particular ```libusb.1.0.so.0```.

If you haven't linked with the correct version of libusb, try running:

```
pkg-config --cflags libusb-1.0
```

OpenCBM XUM1541 uses this within ```opencbm/Linux/config.make``` to configure the build, and will fall bck to libusb-0.1 if it doesn't get a sensible response.  The response should look something like this:

```
-I/usr/include/libusb-1.0
```