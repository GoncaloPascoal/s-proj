
<div align="center">
    <h1>OXID-8: a CHIP-8 / S-CHIP Emulator Developed using the Rust Programming Language</h1>
    <h2>Seminars Project - Group 2</h2>
</div>

## Dependencies

Ensure that you have the following software installed:
- Clang
- Rust compiler and `cargo` (https://rustup.rs)
- RetroArch (https://www.retroarch.com/)

## Compilation

```
cargo build --release
```

## Execution

```
retroarch -L target/release/liboxid_8.so rom.ch8
```

Where `rom.ch8` is the path to the ROM file to be executed.

### Quirks

Certain CHIP-8 programs rely on abnormal instruction behaviour (so-called "quirks") to function properly. These quirks can be enabled from the command line by specifying them after the ROM to load. The following quirks are available:

- `quirk-memory`: instructions that write to or read from RAM no longer increment the I register.
- `quirk-shift`: shift instructions now shift register `VX` directly instead of shifting `VY` and storing the result in `VX` 
- `quirk-collision`: draw sprite instruction now stores the number of sprite rows that collided with an existing sprite or were clipped by the bottom of the screen in register `VF` 
- `quirk-resolution`: switching between resolutions now clears the frame buffer
- `quirk-lores16`: permits drawing 16x16 sprites in low-resolution mode with the DXY0 instruction

As an example, the following command activates both the memory and shift quirks:

```
retroarch -L target/release/liboxid_8.so rom_quirks.ch8 quirk-memory quirk-shift
```
