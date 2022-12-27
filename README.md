
<div align="center">
    <h1>Developing a CHIP-8 Emulator with Rust</h1>
    <h2>Seminars Project - Group 2</h2>
</div>

## Dependencies

Ensure that you have the following dependencies installed:
- Clang
- Rust compiler and `cargo` (https://rustup.rs)
- RetroArch (https://www.retroarch.com/)

## Compilation

```
cargo build --release
```

## Execution

```
retroarch -L target/release/libs_proj.so rom.ch8
```

Where `rom.ch8` is the path to the ROM file to be executed.

### Quirks

Certain CHIP-8 programs rely on abnormal instruction behaviour (so-called "quirks") to function properly. These quirks can be enabled from the command line by specifying them after the ROM to load. The following quirks are available:

- `quirk-memory`: instructions that write to or read from RAM no longer increment the I register.
- `quirk-shift`: shift instructions now shift register `VX` directly instead of shifting `VY` and storing the result in `VX` 

As an example, the following command activates both quirks:

```
retroarch -L target/release/libs_proj.so rom_quirks.ch8 quirk-memory quirk-shift
```
