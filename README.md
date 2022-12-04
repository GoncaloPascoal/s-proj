
<div align="center">
    <h1>Developing a CHIP-8 Emulator with Rust</h1>
    <h2>Seminars Project - Group 2</h2>
</div>

## Compilation

```
cargo build --release
```

## Execution

```
retroarch -L target/release/libs_proj.so rom.ch8
```

Where `rom.ch8` is the path to the ROM file to be executed.