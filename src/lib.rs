
use std::{collections::HashMap, fs};
use rand::Rng;

use libretro_rs::{libretro_core, RetroCore, RetroEnvironment, RetroGame,
    RetroLoadGameResult, RetroRuntime, RetroSystemInfo, RetroAudioInfo,
    RetroVideoInfo};
use bitvec::prelude::*;

use cpu::Cpu;

pub mod cpu;

type FrameBufferRow = BitArr!(for Chip8Core::SCREEN_WIDTH);
type FrameBuffer = [FrameBufferRow; Chip8Core::SCREEN_HEIGHT];

pub struct Chip8Core {
    cpu: Cpu,
    frame_buffer: FrameBuffer,
}

impl Chip8Core {
    const SCREEN_WIDTH: usize = 64;
    const SCREEN_HEIGHT: usize = 32;

    /// Number of video frames to display each second. Typically, a rate of 60Hz is used.
    const FRAME_RATE: f64 = 60.0;
    /// Number of CHIP-8 instruction executed per video frame. Frequency is equal
    /// to `FRAME_RATE` * `INSTRUCTIONS_PER_FRAME`.
    const INSTRUCTIONS_PER_FRAME: usize = 10;

    fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            frame_buffer: [BitArray::ZERO; Chip8Core::SCREEN_HEIGHT],
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

    /// Add `NN` to register `VX`
    fn add(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let n = *args.get("N").unwrap() as u8;

        let x_val = self.cpu.registers[x];

        self.cpu.registers[x] = x_val.wrapping_add(n);
    }

    /// Store value of register `VY` in register `VX`
    fn movr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        self.cpu.registers[x] = self.cpu.registers[y];
    }

    /// Store memory address `NNN` in register `I`
    fn movi(&mut self, args: HashMap<&'static str, u16>) {
        let n = *args.get("N").unwrap() as u16;

        self.cpu.i_register = n;
    }

    /// Add value of register `VX` to register `I`
    fn addi(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let x_val = self.cpu.registers[x] as u16;
        let i_val = self.cpu.i_register;

        self.cpu.i_register = i_val.wrapping_add(x_val);
    }

    /// Store value of `VY` in `VX` shifted right one bit. Set `VF` to least
    /// significant bit prior to shift.
    fn shr(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let y_val = self.cpu.registers[y];

        // Store least significant bit in VF
        self.cpu.registers[0xF] = y_val & 0x01;
        self.cpu.registers[x] = y_val >> 1;
    }

    /// Store value of `VY` in `VX` shifted left one bit. Set `VF` to most
    /// significant bit prior to shift.
    fn shl(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let y = *args.get("Y").unwrap() as usize;

        let y_val = self.cpu.registers[y];

        // Store most significant bit in VF
        self.cpu.registers[0xF] = (y_val & 0x80) >> 7;
        self.cpu.registers[x] = y_val << 1;
    }

    /// Set 'VX' to 'VX' OR 'VY'
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
        let n = *args.get("N").unwrap() as usize;

        let x_val = self.cpu.registers[x] as usize % Self::SCREEN_WIDTH;
        let y_val = self.cpu.registers[y] as usize % Self::SCREEN_HEIGHT;

        // True if a white pixel was set to black when drawing the sprite.
        let mut black = false;

        for i in 0..n {
            if y_val + i == Self::SCREEN_HEIGHT {
                break;
            }

            let sprite_data = self.cpu.memory[self.cpu.i_register as usize + i];
            let row = &mut self.frame_buffer[y_val + i];
            let width = usize::min(8, Self::SCREEN_WIDTH - x_val);

            for j in 0..width {
                let mut screen_bit_ref = row.get_mut(x_val + j).unwrap();
                let sprite_bit = *sprite_data.view_bits::<Msb0>().get(j).unwrap();

                black |= *screen_bit_ref & sprite_bit;
                screen_bit_ref.set(*screen_bit_ref ^ sprite_bit);
            }
        }

        self.cpu.registers[0xF] = black as u8;
    }

    /// Set `VX` to random number with mask `NN`
    fn rand(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;
        let n = *args.get("N").unwrap() as u8;

        let rand: u8 = rand::thread_rng().gen();

        self.cpu.registers[x] = rand & n;
    }

    /// Store values of registers `V0` to `VX` in memory starting at address `I`,
    /// which is set to `I + X + 1` after operation.
    fn save(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let cpu = &mut self.cpu;

        for reg in 0..=x {
            cpu.memory[cpu.i_register as usize] = cpu.registers[reg];
            cpu.i_register += 1;
        }
    }

    /// Fill registers `V0` to `VX` with memory values starting at address I,
    /// which is set to `I + X + 1` after operation.
    fn load(&mut self, args: HashMap<&'static str, u16>) {
        let x = *args.get("X").unwrap() as usize;

        let cpu = &mut self.cpu;

        for reg in 0..=x {
            cpu.registers[reg] = cpu.memory[cpu.i_register as usize];
            cpu.i_register += 1;
        }
    }
}

impl RetroCore for Chip8Core {
    fn init(_env: &RetroEnvironment) -> Self {
        Chip8Core::new()
    }

    fn get_system_info() -> RetroSystemInfo {
        RetroSystemInfo::new("CHIP-8 Emulator", "0.1.0")
    }

    fn reset(&mut self, _env: &RetroEnvironment) {

    }

    fn run(&mut self, _env: &RetroEnvironment, runtime: &RetroRuntime) {
        // TODO: Get input from user

        for _ in 0..Self::INSTRUCTIONS_PER_FRAME {
            self.execute_instruction();
        }

        let mut frame = [0; 2 * Self::SCREEN_WIDTH * Self::SCREEN_HEIGHT];
        let mut i = 0;

        for row in &self.frame_buffer {
            for bit in row {
                if *bit {
                    frame[i] = 0xFF;
                    frame[i + 1] = 0xFF;
                }
                i += 2;
            }
        }

        runtime.upload_video_frame(&frame, Self::SCREEN_WIDTH as u32,
            Self::SCREEN_HEIGHT as u32, 2 * Self::SCREEN_WIDTH);

        // TODO: Upload audio frames
    }

    fn load_game(&mut self, _env: &RetroEnvironment, game: RetroGame) -> RetroLoadGameResult {
        let mut program_data = Vec::new();

        match game {
            RetroGame::None { meta: _ } => return RetroLoadGameResult::Failure,
            RetroGame::Data { meta: _, data } => program_data.extend_from_slice(data),
            RetroGame::Path { meta: _, path } => {
                if let Ok(data) = fs::read(path) {
                    program_data = data;
                } else {
                    return RetroLoadGameResult::Failure;
                }
            },
        }

        self.cpu.load_program(program_data.as_slice());

        RetroLoadGameResult::Success {
            audio: RetroAudioInfo::new(0.0),
            video: RetroVideoInfo::new(Self::FRAME_RATE, 64, 32),
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
}
