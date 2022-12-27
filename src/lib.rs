
use std::{collections::HashMap, fs::File, fs::read, io::Write, io::Read, process, env};
use bitvec::{prelude::Msb0, view::BitView};
use rand::Rng;

use libretro_rs::{libretro_core, RetroCore, RetroEnvironment, RetroGame,
    RetroLoadGameResult, RetroRuntime, RetroSystemInfo, RetroAudioInfo,
    RetroVideoInfo, RetroPixelFormat, RetroRegion, RetroDevicePort};
use strum::IntoEnumIterator;

use cpu::Cpu;
use input::Chip8Key;

pub mod cpu;
pub mod input;

type FrameBuffer = [[bool; Chip8Core::SCREEN_WIDTH]; Chip8Core::SCREEN_HEIGHT];

pub struct Chip8Core {
    cpu: Cpu,
    frame_buffer: FrameBuffer,
    high_resolution: bool,
    keypad_state: [bool; Self::KEYPAD_SIZE],
    wave: [i16; 2 * Self::SAMPLE_RATE as usize],
    wave_idx: usize,
    quirk_memory: bool,
    quirk_shift: bool,
}

fn sample_square_wave(amplitude: i16, frequency: f64, t: f64) -> i16 {
    amplitude * i16::pow(-1, (2.0 * frequency * t).floor() as u32)
}

impl Chip8Core {
    const SCREEN_WIDTH: usize = 128;
    const SCREEN_HEIGHT: usize = 64;

    /// RGB565 representation of the white (on) pixel color.
    const WHITE_COLOR: u16 = 0x9DE2;
    /// RGB565 representation of the black (off) pixel color.
    const BLACK_COLOR: u16 = 0x11C2;

    const DIGIT_SIZE: usize = 5;
    const LARGE_DIGIT_SIZE: usize = 10;
    const LARGE_DIGIT_OFFSET: usize = 128;

    const FLAGS_FILE: &'static str = "flags.rpl";

    /// Number of video frames to display each second. Typically, a rate of 60Hz is used.
    const FRAME_RATE: f64 = 60.0;
    /// Number of CHIP-8 instruction executed per video frame. Frequency is equal
    /// to `FRAME_RATE` * `INSTRUCTIONS_PER_FRAME`.
    const INSTRUCTIONS_PER_FRAME: usize = 10;

    /// Audio sample rate in Hertz.
    const SAMPLE_RATE: f64 = 48000.0;
    /// Size of a single audio frame in bytes.
    const AUDIO_FRAME_SIZE: usize = 2 * (Self::SAMPLE_RATE / Self::FRAME_RATE) as usize;
    /// Amplitude of the square wave.
    const WAVE_AMPLITUDE: i16 = 1200;
    /// Frequency of the square wave. For best results, this value should divide
    /// the audio sample rate.
    const WAVE_FREQUENCY: f64 = 250.0;
    /// Maximum value of the wave_idx member field.
    const MAX_WAVE_IDX: usize = Self::SAMPLE_RATE as usize / Self::AUDIO_FRAME_SIZE;

    const KEYPAD_SIZE: usize = 16;

    fn new() -> Self {
        Self::with_quirks(false, false)
    }

    fn with_quirks(memory: bool, shift: bool) -> Self {
        // Precalculate square wave to decrease required computation.
        let mut wave = [0; 2 * Self::SAMPLE_RATE as usize];
        for (i, sample) in wave.iter_mut().enumerate() {
            *sample = sample_square_wave(Self::WAVE_AMPLITUDE, Self::WAVE_FREQUENCY, i as f64 / Self::SAMPLE_RATE); 
        }

        Self {
            cpu: Cpu::new(),
            frame_buffer: [[false; Chip8Core::SCREEN_WIDTH]; Chip8Core::SCREEN_HEIGHT],
            high_resolution: false,
            keypad_state: [false; Self::KEYPAD_SIZE],
            wave,
            wave_idx: 0,
            quirk_memory: memory,
            quirk_shift: shift,
        }
    }

    fn execute_instruction(&mut self) {
        let raw_instruction = self.cpu.fetch_instruction();
        let instruction = self.cpu.decode_instruction(raw_instruction);

        (instruction.callback)(self, instruction.args(raw_instruction));
    }

    /// No operation.
    fn nop(&mut self, _args: HashMap<&'static str, u16>) {

    }

    /// Clear the screen.
    fn cls(&mut self, _args: HashMap<&'static str, u16>) {
        for row in &mut self.frame_buffer {
            row.fill(false);
        }
    }

    /// Jump to address `NNN`.
    fn jmp(&mut self, args: HashMap<&'static str, u16>) {
        let n = *args.get("N").unwrap();

        self.cpu.pc = n;
    }

    /// Execute subroutine starting at address `NNN`.
    fn call(&mut self, args: HashMap<&'static str, u16>) {
        let n = *args.get("N").unwrap();

        self.cpu.stack.push(self.cpu.pc);
        self.cpu.pc = n;
    }

    /// Return from a subroutine.
    fn ret(&mut self, _args: HashMap<&'static str, u16>) {
        if let Some(stack_top) = self.cpu.stack.pop() {
            self.cpu.pc = stack_top;
        }
    }

    /// Scroll display down by `N` pixels, or `N/2` pixels in low-resolution mode.
    /// **SUPER-CHIP instruction.**
    fn scd(&mut self, args: HashMap<&'static str, u16>) {
        let n = *args.get("N").unwrap() as usize % Self::SCREEN_HEIGHT;

        let mut new_buffer = [[false; Chip8Core::SCREEN_WIDTH]; Chip8Core::SCREEN_HEIGHT];
        new_buffer[n..].copy_from_slice(&self.frame_buffer[..Chip8Core::SCREEN_HEIGHT - n]);
        self.frame_buffer = new_buffer;
    }

    /// Scroll display right by 4 pixels, or 2 in low-resolution mode. **SUPER-CHIP instruction.**
    fn scr(&mut self, _args: HashMap<&'static str, u16>) {
        let pixels = 4;

        for row in &mut self.frame_buffer {
            let mut new_row = [false; Chip8Core::SCREEN_WIDTH];
            new_row[pixels..].copy_from_slice(&row[..Chip8Core::SCREEN_WIDTH - pixels]);
            *row = new_row;
        }
    }

    /// Scroll display left by 4 pixels, or 2 in low-resolution mode. **SUPER-CHIP instruction.**
    fn scl(&mut self, _args: HashMap<&'static str, u16>) {
        let pixels = 4;

        for row in &mut self.frame_buffer {
            let mut new_row = [false; Chip8Core::SCREEN_WIDTH];
            new_row[..Chip8Core::SCREEN_WIDTH - pixels].copy_from_slice(&row[pixels..]);
            *row = new_row;
        }
    }

    /// Exit the interpreter. **SUPER-CHIP instruction.**
    fn exit(&mut self, _args: HashMap<&'static str, u16>) {
        process::exit(0);
    }

    /// Disable high-resolution mode. **SUPER-CHIP instruction.**
    fn lores(&mut self, _args: HashMap<&'static str, u16>) {
        self.high_resolution = false;
    }

    /// Enable high-resolution mode. **SUPER-CHIP instruction.**
    fn hires(&mut self, _args: HashMap<&'static str, u16>) {
        self.high_resolution = true;
    }
    
    /// Skip following instruction if value of register `VX` equals `NN`.
    fn skpeq(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let n = *args.get("N").unwrap() as u8;

        let x_val = self.cpu.registers[x];

        if x_val == n {
            self.cpu.pc += 2;
        }
    }

    /// Skip following instruction if value of register `VX` does not equals `NN`.
    fn skpne(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let n = *args.get("N").unwrap() as u8;

        let x_val = self.cpu.registers[x];

        if x_val != n {
            self.cpu.pc += 2;
        }
    }

    /// Skip following instruction if value of register `VX` is equal to value of register `VY`.
    fn skpeqr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let x_val = self.cpu.registers[x];
        let y_val = self.cpu.registers[y];

        if x_val == y_val {
            self.cpu.pc += 2;
        }
    }

    /// Skip following instruction if value of register `VX` is not equal to `VY`.
    fn skpner(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let x_val = self.cpu.registers[x];
        let y_val = self.cpu.registers[y];

        if x_val != y_val {
            self.cpu.pc += 2;
        }
    }

    /// Jump to address `NNN + V0`.
    fn jmpr(&mut self, args: HashMap<&'static str, u16>) {
        let n = *args.get("N").unwrap();
        let reg_val = self.cpu.registers[0x0] as u16;
        let mem_size = self.cpu.memory.len() as u16;

        self.cpu.pc = (n + reg_val) % mem_size;
    }

    /// Add value of register `VY` to register `VX`. Set `VF` to `01` if carry
    /// occurs, `00` otherwise.
    fn addr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let x_val = self.cpu.registers[x];
        let y_val = self.cpu.registers[y];

        let (result, carry) = x_val.overflowing_add(y_val);

        self.cpu.registers[x] = result;
        self.cpu.registers[0xF] = carry as u8;
    }

    /// Subtract value of register `VY` from register `VX`. Set `VF` to `00` if a borrow
    /// occurs, `01` otherwise.
    fn subr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let x_val = self.cpu.registers[x];
        let y_val = self.cpu.registers[y];

        let (result, borrow) = x_val.overflowing_sub(y_val);

        self.cpu.registers[x] = result;
        self.cpu.registers[0xF] = !borrow as u8;
    }

    /// Set `VX` to value of `VY` minus `VX`. Set `VF` to `00` if a borrow
    /// occurs, `01` otherwise.
    fn rsubr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let x_val = self.cpu.registers[x];
        let y_val = self.cpu.registers[y];

        let (result, borrow) = y_val.overflowing_sub(x_val);

        self.cpu.registers[x] = result;
        self.cpu.registers[0xF] = !borrow as u8;
    }

    /// Store `NN` in register `VX`.
    fn mov(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let n = *args.get("N").unwrap() as u8;

        self.cpu.registers[x] = n;
    }

    /// Add `NN` to register `VX`.
    fn add(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let n = *args.get("N").unwrap() as u8;

        let x_val = self.cpu.registers[x];

        self.cpu.registers[x] = x_val.wrapping_add(n);
    }

    /// Store value of register `VY` in register `VX`.
    fn movr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        self.cpu.registers[x] = self.cpu.registers[y];
    }

    /// Store memory address `NNN` in register `I`.
    fn movi(&mut self, args: HashMap<&'static str, u16>) {
        let n = *args.get("N").unwrap();

        self.cpu.i_register = n;
    }

    /// Set sound timer to value of register `VX`.
    fn sndr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        self.cpu.sound_timer = self.cpu.registers[x];
    }

    /// Store current value of delay timer in register `VX`.
    fn timr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        self.cpu.registers[x] = self.cpu.delay_timer;
    }

    /// Set delay timer to value of register `VX`.
    fn delr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        self.cpu.delay_timer = self.cpu.registers[x];
    }

    /// Set `I` to memory address of 5-byte sprite data corresponding to hex digit stored in register `VX`.
    fn digit(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let x_val = self.cpu.registers[x] as usize % Self::KEYPAD_SIZE;
        self.cpu.i_register = (x_val * Self::DIGIT_SIZE) as u16;
    }

    /// Set I to memory address of 10-byte sprite data corresponding to  hex digit stored in register VX.
    /// Only digits 0-9 have high-resolution sprite representations. **SUPER-CHIP instruction.**
    fn ldigit(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let x_val = self.cpu.registers[x] as usize % Self::KEYPAD_SIZE;
        self.cpu.i_register = (Self::LARGE_DIGIT_OFFSET + x_val * Self::LARGE_DIGIT_SIZE) as u16;
    }

    /// Add value of register `VX` to register `I`.
    fn addi(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let x_val = self.cpu.registers[x] as u16;
        let i_val = self.cpu.i_register;

        self.cpu.i_register = i_val.wrapping_add(x_val);
    }

    /// Wait for keypress and store result in register `VX`.
    fn key(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        self.cpu.store_keypress = Some(x);
    }

    // Skip following instruction if key corresponding to hex value in `VX` is pressed.
    fn skpk(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let x_val = self.cpu.registers[x] as usize % Self::KEYPAD_SIZE;
        
        if self.keypad_state[x_val] {
            self.cpu.pc += 2;
        }
    }

    // Skip following instruction if key corresponding to hex value in `VX` is not pressed.
    fn skpnk(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let x_val = self.cpu.registers[x] as usize % Self::KEYPAD_SIZE;
        
        if !self.keypad_state[x_val] {
            self.cpu.pc += 2;
        }
    }

    /// Store value of `VY` in `VX` shifted right one bit. Set `VF` to least
    /// significant bit prior to shift. `VX` is shifted instead if the "shift" quirk is active.
    fn shr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let y_val = if self.quirk_shift { self.cpu.registers[x] } else { self.cpu.registers[y] };

        // Store least significant bit in VF
        self.cpu.registers[0xF] = y_val & 0x01;
        self.cpu.registers[x] = y_val >> 1;
    }

    /// Store value of `VY` in `VX` shifted left one bit. Set `VF` to most
    /// significant bit prior to shift. `VX` is shifted instead if the "shift" quirk is active.
    fn shl(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let y_val = if self.quirk_shift { self.cpu.registers[x] } else { self.cpu.registers[y] };

        // Store most significant bit in VF
        self.cpu.registers[0xF] = (y_val & 0x80) >> 7;
        self.cpu.registers[x] = y_val << 1;
    }

    /// Set 'VX' to 'VX' OR 'VY'.
    fn or(&mut self, args: HashMap<&'static str, u16>) {
        let x: usize = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        self.cpu.registers[x] |= self.cpu.registers[y];
    }

    /// Set `VX` to `VX` AND `VY`.
    fn and(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        self.cpu.registers[x] &= self.cpu.registers[y];
    }

    /// Set `VX` to `VX` XOR `VY`.
    fn xor(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        self.cpu.registers[x] ^= self.cpu.registers[y];
    }

    /// Draw a sprite at `(VX, VY)` with `N` bytes of sprite data starting at
    /// address stored in `I`. Set `VF` to `01` if any pixels are set to black,
    /// `00` otherwise.
    fn draw(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;
        let mut n = *args.get("N").unwrap() as usize;

        let mut columns = 8;
        let draw_large_sprite = self.high_resolution && n == 0;
        let addr_scaling_factor = draw_large_sprite as usize + 1;

        if draw_large_sprite {
            n = 16;
            columns = 16;
        }

        let mut x_val = self.cpu.registers[x] as usize;
        if !self.high_resolution { x_val *= 2; }
        x_val %= Self::SCREEN_WIDTH;

        let mut y_val = self.cpu.registers[y] as usize;
        if !self.high_resolution { y_val *= 2; }
        y_val %= Self::SCREEN_HEIGHT;

        /* In low resolution mode, equal to 0x01 if a white pixel was set to black when drawing the sprite.
           In high resolution mode, equal to the number of sprite rows where this occurred or that were clipped
           by the bottom of the screen. */
        let mut black = 0x00;

        let scaling_factor = !self.high_resolution as usize + 1;

        let height = usize::min(n, (Self::SCREEN_HEIGHT - y_val) / scaling_factor);
        for i in 0..height {
            let mut row_black = false;

            let addr = self.cpu.i_register as usize + i * addr_scaling_factor;
            let sprite_data = u16::from_be_bytes(
                if draw_large_sprite {
                    self.cpu.memory[addr..=addr + 1].try_into().unwrap()
                }
                else {
                    [self.cpu.memory[addr], 0x00]
                }
            );

            for offset_i in 0..scaling_factor {
                let row = &mut self.frame_buffer[y_val + i * scaling_factor + offset_i];
                let width = usize::min(columns, (Self::SCREEN_WIDTH - x_val) / scaling_factor);

                for j in 0..width {
                    let sprite_bit = *sprite_data.view_bits::<Msb0>().get(j).unwrap();

                    for offset_j in 0..scaling_factor {
                        let screen_bit_ref = &mut row[x_val + j * scaling_factor + offset_j];

                        row_black |= *screen_bit_ref && sprite_bit;
                        *screen_bit_ref ^= sprite_bit;
                    }
                }
            }

            if self.high_resolution {
                black += row_black as u8;
            }
            else {
                black |= row_black as u8;
            }
        }

        black += (n - height) as u8;
        self.cpu.registers[0xF] = black;
    }

    /// Set `VX` to random number with mask `NN`.
    fn rand(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let n = *args.get("N").unwrap() as u8;

        let rand: u8 = rand::thread_rng().gen();

        self.cpu.registers[x] = rand & n;
    }

    /// Store BCD equivalent of value stored in register `VX` in memory at
    /// addresses `I` to `I + 2`.
    fn bcd(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let cpu = &mut self.cpu;
        let x_val = cpu.registers[x];

        for i in 0..=2 {
            let addr = cpu.i_register as usize + i;
            let digit = (x_val / u8::pow(10, 2 - i as u32)) % 10;

            cpu.memory[addr] = digit;
        }
    }

    /// Store values of registers `V0` to `VX` in memory starting at address `I`,
    /// which is set to `I + X + 1` after operation (unless the "memory" quirk is active).
    fn save(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let cpu = &mut self.cpu;

        for reg in 0..=x {
            cpu.memory[cpu.i_register as usize + reg] = cpu.registers[reg];
        }

        if !self.quirk_memory {
            cpu.i_register += x as u16 + 1;
        }
    }

    /// Fill registers `V0` to `VX` with memory values starting at address I,
    /// which is set to `I + X + 1` after operation (unless the "memory" quirk is active).
    fn load(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let cpu = &mut self.cpu;

        for reg in 0..=x {
            cpu.registers[reg] = cpu.memory[cpu.i_register as usize + reg];
        }

        if !self.quirk_memory {
            cpu.i_register += x as u16 + 1;
        }
    }

    /// Store values of register `V0` to `VX` from RPL user flags (persistent memory).
    /// `X` must be less than or equal to 7. **SUPER-CHIP instruction.**
    fn savef(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        if x > 7 { return; }

        if let Ok(mut file) = File::create(Self::FLAGS_FILE) {
            let _ = file.write_all(&self.cpu.registers[0..=x]);
        }
    }

    /// Load values of registers `V0` to `VX` to RPL user flags (persistent memory).
    /// `X` must be less than or equal to 7. **SUPER-CHIP instruction.**
    fn loadf(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        if x > 7 { return; }

        if let Ok(mut file) = File::open(Self::FLAGS_FILE) {
            let _ = file.read_exact(self.cpu.registers[0..=x].as_mut());
        }
    }
}

impl RetroCore for Chip8Core {
    fn get_system_info() -> RetroSystemInfo {
        RetroSystemInfo::new("CHIP-8 Emulator", "0.1.0")
    }

    fn reset(&mut self, _env: &mut RetroEnvironment) {

    }

    fn run(&mut self, _env: &mut RetroEnvironment, runtime: &RetroRuntime) {
        let port = 0;

        // Obtain user input
        for (i, key) in Chip8Key::iter().enumerate() {
            self.keypad_state[i] = runtime.is_keyboard_key_pressed(
                RetroDevicePort::new(port),
                key as u32
            );
        }

        // Update timers
        let delay_timer = &mut self.cpu.delay_timer;
        let sound_timer = &mut self.cpu.sound_timer;

        *delay_timer = delay_timer.saturating_sub(1);
        *sound_timer = sound_timer.saturating_sub(1);

        for _ in 0..Self::INSTRUCTIONS_PER_FRAME {
            if self.cpu.store_keypress.is_some() {
                break;
            }
            self.execute_instruction();
        }

        if let Some(reg) = self.cpu.store_keypress {
            if let Some(val) = self.keypad_state.iter().position(|&pressed| pressed) {
                self.cpu.registers[reg] = val as u8;
                self.cpu.store_keypress = None;
            }
        }

        let mut frame = [0; 2 * Self::SCREEN_WIDTH * Self::SCREEN_HEIGHT];
        let mut i = 0;

        for row in &self.frame_buffer {
            for bit in row {
                if *bit {
                    frame[i..=i + 1].clone_from_slice(&Self::WHITE_COLOR.to_le_bytes());
                }
                else {
                    frame[i..=i + 1].clone_from_slice(&Self::BLACK_COLOR.to_le_bytes());
                }
                i += 2;
            }
        }

        runtime.upload_video_frame(&frame, Self::SCREEN_WIDTH as u32,
            Self::SCREEN_HEIGHT as u32, 2 * Self::SCREEN_WIDTH);

        let idx = self.wave_idx * Self::AUDIO_FRAME_SIZE;
        self.wave_idx += 1;
        self.wave_idx %= Self::MAX_WAVE_IDX;

        if self.cpu.sound_timer != 0 {
            let audio_frame = &self.wave[idx..idx + Self::AUDIO_FRAME_SIZE];
            runtime.upload_audio_frame(audio_frame);
        }
    }

    fn load_game(_env: &mut RetroEnvironment, game: RetroGame) -> RetroLoadGameResult<Self> {
        let args: Vec<String> = env::args().collect();

        // Quirks
        let memory = args.iter().any(|s| s == "quirk-memory");
        let shift = args.iter().any(|s| s == "quirk-shift");
        
        let mut core = Chip8Core::with_quirks(memory, shift);
        let program_data;

        match game {
            RetroGame::None { meta: _ } => return RetroLoadGameResult::Failure,
            RetroGame::Data { meta: _, data, path: _ } => program_data = data,
            RetroGame::Path { meta: _, path } => {
                if let Ok(data) = read(path) {
                    program_data = data;
                } else {
                    return RetroLoadGameResult::Failure;
                }
            },
        }

        core.cpu.load_program(program_data.as_slice());

        RetroLoadGameResult::Success {
            region: RetroRegion::NTSC,
            audio: RetroAudioInfo::new(Self::SAMPLE_RATE),
            video: RetroVideoInfo::new(Self::FRAME_RATE, 64, 32)
                .with_pixel_format(RetroPixelFormat::RGB565),
            core,
        }
    }
}

libretro_core!(Chip8Core);


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add() {
        let mut core = Chip8Core::new();

        core.cpu.registers[0x2] = 200;
        
        core.add(HashMap::from([("X", 0x2), ("N", 100)]));

        assert_eq!(core.cpu.registers[0x2], 44);
    }

    #[test]
    fn addr() {
        let mut core = Chip8Core::new();

        core.cpu.registers[0x2] = 25;
        core.cpu.registers[0x3] = 42;
        core.cpu.registers[0xF] = 33;

        core.addr(HashMap::from([("X", 0x2), ("Y", 0x3)]));

        assert_eq!(core.cpu.registers[0x2], 67);
        assert_eq!(core.cpu.registers[0xF], 0);

        core.cpu.registers[0x2] = 255;
        core.cpu.registers[0x3] = 20;

        core.addr(HashMap::from([("X", 0x2), ("Y", 0x3)]));

        assert_eq!(core.cpu.registers[0x2], 19);
        assert_eq!(core.cpu.registers[0xF], 1);
    }

    #[test]
    fn movi() {
        let mut core = Chip8Core::new();
        let addr = 0x34E;

        core.movi(HashMap::from([("N", addr)]));

        assert_eq!(core.cpu.i_register, addr);
    }

    #[test]
    fn rsubr() {
        let mut core = Chip8Core::new();

        core.cpu.registers[0x2] = 31;
        core.cpu.registers[0x3] = 65;
        core.cpu.registers[0xF] = 33;

        core.rsubr(HashMap::from([("X", 0x2), ("Y", 0x3)]));

        assert_eq!(core.cpu.registers[0x2], 34);
        assert_eq!(core.cpu.registers[0xF], 1);

        core.cpu.registers[0x2] = 31;
        core.cpu.registers[0x3] = 20;

        core.rsubr(HashMap::from([("X", 0x2), ("Y", 0x3)]));

        assert_eq!(core.cpu.registers[0x2], 245);
        assert_eq!(core.cpu.registers[0xF], 0);
    }

    #[test]
    fn shl() {
        let mut core = Chip8Core::new();

        core.cpu.registers[0x2] = 0x01;
        core.cpu.registers[0xF] = 33;

        core.shl(HashMap::from([("X", 0x1), ("Y", 0x2)]));

        assert_eq!(core.cpu.registers[0x1], 0x2);
        assert_eq!(core.cpu.registers[0xF], 0x0);

        core.cpu.registers[0x2] = 0x81;

        core.shl(HashMap::from([("X", 0x1), ("Y", 0x2)]));

        assert_eq!(core.cpu.registers[0x1], 0x2);
        assert_eq!(core.cpu.registers[0xF], 0x1);
    }

    #[test]
    fn jmpr() {
        let mut core = Chip8Core::new();

        core.cpu.registers[0x0] = 0x40;

        core.jmpr(HashMap::from([("N", 0x300)]));

        assert_eq!(core.cpu.pc, 0x340);
    }

    #[test]
    fn call_ret() {
        let mut core = Chip8Core::new();

        let pc = 0x432;
        let addr = 0x6A2;

        core.cpu.pc = pc;
        core.call(HashMap::from([("N", addr)]));

        assert_eq!(core.cpu.pc, addr);
        assert_eq!(core.cpu.stack, vec![pc]);

        core.ret(HashMap::new());

        assert_eq!(core.cpu.pc, pc);
        assert_eq!(core.cpu.stack, Vec::new());
    }

    #[test]
    fn skpeqr() {
        let mut core = Chip8Core::new();

        let pc = 0x3A0;
        core.cpu.pc = pc;

        let v = vec![0x42, 0x34, 0x42];
        core.cpu.registers[0x0] = v[0];
        core.cpu.registers[0x1] = v[1];
        core.cpu.registers[0x2] = v[2];

        core.skpeqr(HashMap::from([("X", 0x0), ("Y", 0x1)]));
        assert_eq!(core.cpu.pc, pc);

        core.skpeqr(HashMap::from([("X", 0x0), ("Y", 0x2)]));
        assert_eq!(core.cpu.pc, pc + 2);
    }

    #[test]
    fn skpk() {
        let mut core = Chip8Core::new();

        let pc = 0x3A0;
        core.cpu.pc = pc;
        
        let key = 0xB;
        core.keypad_state[key] = true;

        core.cpu.registers[0x0] = 0x8;
        core.skpk(HashMap::from([("X", 0x0)]));
        assert_eq!(core.cpu.pc, pc);

        core.cpu.registers[0x0] = 0xB;
        core.skpk(HashMap::from([("X", 0x0)]));
        assert_eq!(core.cpu.pc, pc + 2);
    }

    #[test]
    fn timr() {
        let mut core = Chip8Core::new();

        let val = 0x7A;
        core.cpu.delay_timer = val;

        core.timr(HashMap::from([("X", 0x2)]));
        assert_eq!(core.cpu.registers[0x2], val);
    }

    #[test]
    fn bcd() {
        let mut core = Chip8Core::new();

        let i = 0x400 as usize;
        core.cpu.i_register = i as u16;

        core.cpu.registers[0x4] = 159;

        core.bcd(HashMap::from([("X", 0x4)]));

        assert_eq!(core.cpu.memory[i], 1);
        assert_eq!(core.cpu.memory[i + 1], 5);
        assert_eq!(core.cpu.memory[i + 2], 9);
    }

    #[test]
    fn save() {
        let mut core = Chip8Core::new();

        let i = 0x400 as usize;
        let v = vec![0x41, 0x9B, 0xEE];

        core.cpu.i_register = i as u16;

        core.cpu.registers[0x0] = v[0];
        core.cpu.registers[0x1] = v[1];
        core.cpu.registers[0x2] = v[2];

        core.save(HashMap::from([("X", 0x2)]));

        assert_eq!(core.cpu.memory[i], v[0]);
        assert_eq!(core.cpu.memory[i + 1], v[1]);
        assert_eq!(core.cpu.memory[i + 2], v[2]);

        assert_eq!(core.cpu.i_register, (i + 3) as u16);
    }

    #[test]
    fn load() {
        let mut core = Chip8Core::new();

        let i = 0x400 as usize;
        let v = vec![0x20, 0x45, 0xAF];

        core.cpu.i_register = i as u16;

        core.cpu.memory[i] = v[0];
        core.cpu.memory[i + 1] = v[1];
        core.cpu.memory[i + 2] = v[2];

        core.load(HashMap::from([("X", 0x2)]));

        assert_eq!(core.cpu.registers[0x0], v[0]);
        assert_eq!(core.cpu.registers[0x1], v[1]);
        assert_eq!(core.cpu.registers[0x2], v[2]);

        assert_eq!(core.cpu.i_register, (i + 3) as u16);
    }
}
