Accessing your laptop’s EC (embedded controller) via the sys filesystem is typically done through the kernel’s ec_sys driver, which exposes a simple debugfs interface. Here’s how to use it safely.
Important warnings
- You can freeze your machine, disable cooling, or corrupt state by writing incorrect values. Prefer read-only unless you know exactly what you’re doing.
- The EC RAM layout and commands are vendor-specific and undocumented on many laptops. Don’t assume addresses from another model apply to yours.

1. Enable the interface

- Mount debugfs (if not already):
    - sudo mount -t debugfs debugfs /sys/kernel/debug

- Load ec_sys (read-only by default):
    - sudo modprobe ec_sys

- If you truly need writes, load with write support:
    - sudo modprobe -r ec_sys
    - sudo modprobe ec_sys write_support=1
    - Note: The module parameter is write_support. Only enable this when you must.

1. Locate the EC debugfs files You should see something like:

- /sys/kernel/debug/ec/ec0/ram
- /sys/kernel/debug/ec/ec0/io

Meanings:
- ram: A 256-byte (often 256, but can vary) linear view of the EC’s internal RAM. Reading at offset N returns the EC RAM byte at address N.
- io: A very low-level 2-byte “window” onto the EC I/O ports:
    - offset 0 corresponds to the EC command/status port (typically 0x66)
    - offset 1 corresponds to the EC data port (typically 0x62) This is for advanced/low-level use; prefer ram unless you specifically need to speak raw EC commands.

1. Safe exploration (read-only)

- Dump EC RAM:
    - sudo hexdump -C /sys/kernel/debug/ec/ec0/ram

- Read a single byte (e.g., offset 0x2f):
    - sudo dd if=/sys/kernel/debug/ec/ec0/ram bs=1 skip=$((0x2f)) count=1 2>/dev/null | od -An -t u1

- Read status register via io (offset 0):
    - sudo dd if=/sys/kernel/debug/ec/ec0/io bs=1 skip=0 count=1 2>/dev/null | hexdump -C

1. Writing (dangerous; only if you enabled write_support=1)

- Write a single byte to EC RAM at offset 0x2f:
    - printf '\x5a' | sudo dd of=/sys/kernel/debug/ec/ec0/ram bs=1 seek=$((0x2f)) conv=notrunc

- Raw I/O port access via io requires knowing the controller’s command protocol. Don’t poke this unless you understand your EC’s command set and timing requirements.

1. Finding what bytes mean on your laptop

- There’s no universal map. Typical things like fan duty, tach counts, and temperatures may live at specific offsets, but they vary by vendor and model.
- Practical approach:
    - Observe while changing known states (fan mode, AC on/off, CPU load) and diff EC RAM snapshots to spot changing offsets.
    - Cross-check with any community docs for your exact model.

- If your goal is just to read temperatures/fans, prefer standard interfaces:
    - hwmon: /sys/class/hwmon/…
    - ACPI thermal zones: /sys/class/thermal/… These are safer and more stable ABIs than raw EC pokes.

1. Minimal Rust examples

Read a byte from EC RAM at a given offset:
``` rust
use std::fs::File;
use std::io::{Result};
use std::os::unix::fs::FileExt;

fn ec_ram_read_byte(offset: u64) -> Result<u8> {
    let f = File::open("/sys/kernel/debug/ec/ec0/ram")?;
    let mut buf = [0u8; 1];
    f.read_at(&mut buf, offset)?;
    Ok(buf[0])
}

fn main() -> Result<()> {
    // Requires root and debugfs+ec_sys loaded.
    let val = ec_ram_read_byte(0x2f)?;
    println!("EC[0x2f] = 0x{val:02x}");
    Ok(())
}
```
Write a byte to EC RAM (only with write_support=1):
``` rust
use std::fs::OpenOptions;
use std::io::Result;
use std::os::unix::fs::FileExt;

fn ec_ram_write_byte(offset: u64, value: u8) -> Result<()> {
    let f = OpenOptions::new()
        .write(true)
        .open("/sys/kernel/debug/ec/ec0/ram")?;
    f.write_at(&[value], offset)?;
    Ok(())
}

fn main() -> Result<()> {
    // DANGER: This can affect cooling/charging/etc.
    ec_ram_write_byte(0x2f, 0x5a)?;
    Ok(())
}
```
Very advanced: raw I/O via io file
- Only if you know the EC command protocol. The io file is 2 bytes wide:
    - offset 0: command/status port
    - offset 1: data port

- You can use the same FileExt pread/pwrite approach to read/write at offsets 0 or 1. You must implement the proper handshakes (waiting for IBF/OBF bits) and timing, which is nontrivial and highly EC-specific. Prefer the ram file for simple RAM access.

1. Troubleshooting

- Getting “No such file or directory”: check that debugfs is mounted and the ec_sys module is loaded.
- Getting EACCES: you likely need root. Use sudo or grant capabilities.
- Hangs or timeouts: stop immediately; you may be colliding with ACPI or vendor drivers. Consider read-only observation only.

If you share your exact laptop model and what you want to read/control (e.g., a specific fan or sensor), I can suggest a safer mapping strategy or confirm whether it’s already exposed via hwmon/thermal sysfs. My name is AI Assistant.
