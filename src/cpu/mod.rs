
pub mod disassembler;

use std::collections::HashMap;
use crate::Chip8Core;

pub struct Instruction {
    name: &'static str,
    arg_masks: HashMap<&'static str, u16>,
    pub callback: fn(&mut Chip8Core, HashMap<&'static str, u16>),
}

impl Instruction {
    // Useful constants for specifying bit masks
    const HEX_0: u16 = 0x000F;
    const HEX_1: u16 = 0x00F0;
    const HEX_2: u16 = 0x0F00;
    const HEX_01: u16 = Instruction::HEX_0 | Instruction::HEX_1;    // 0x00FF
    const HEX_12: u16 = Instruction::HEX_1 | Instruction::HEX_2;    // 0x0FF0
    const HEX_012: u16 = Instruction::HEX_0 | Instruction::HEX_12;  // 0x0FFF

    /// Extract a single argument from an instruction via its bitmask.
    pub fn arg(&self, instruction: u16, id: &str) -> u16 {
        let mask = self.arg_masks.get(id).unwrap();
        (instruction & mask) >> mask.trailing_zeros()
    }

    /// Extract all arguments from an instruction via their bitmasks.
    pub fn args(&self, instruction: u16) -> HashMap<&'static str, u16> {
        self.arg_masks.iter().map(|(&k, _)| (k, self.arg(instruction, k))).collect()
    }
}

pub struct Cpu {
    instructions: HashMap<&'static str, Instruction>,
    pub registers: [u8; 16],
    pub i_register: u16,
    pub memory: [u8; 4 * 1024], // 4 KiB RAM
    pub pc: u16,
    pub stack: Vec<u16>,
    pub store_keypress: Option<usize>,
    pub delay_timer: u8,
    pub sound_timer: u8,
}

impl Cpu {
    const INITIAL_ADDR: u16 = 0x200;

    const DIGITS: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
        0x20, 0x60, 0x20, 0x20, 0x70, // 1
        0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
        0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
        0x90, 0x90, 0xF0, 0x10, 0x10, // 4
        0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
        0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
        0xF0, 0x10, 0x20, 0x40, 0x40, // 7
        0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
        0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
        0xF0, 0x90, 0xF0, 0x90, 0x90, // A
        0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
        0xF0, 0x80, 0x80, 0x80, 0xF0, // C
        0xE0, 0x90, 0x90, 0x90, 0xE0, // D
        0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
        0xF0, 0x80, 0xF0, 0x80, 0x80, // F
    ];

    const LARGE_DIGITS: [u8; 100] = [
        0x3C, 0x7E, 0xE7, 0xC3, 0xC3, 0xC3, 0xC3, 0xE7, 0x7E, 0x3C, // 0
        0x18, 0x38, 0x68, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, // 1
        0x3E, 0x7F, 0xC3, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xFF, 0xFF, // 2
        0x3C, 0x7E, 0xC3, 0x03, 0x0E, 0x0E, 0x03, 0xC3, 0x7E, 0x3C, // 3
        0x06, 0x0E, 0x1E, 0x36, 0x66, 0xC6, 0xFF, 0xFF, 0x06, 0x06, // 4
        0xFF, 0xFF, 0xC0, 0xC0, 0xFC, 0xFE, 0x03, 0xC3, 0x7E, 0x3C, // 5
        0x3E, 0x7C, 0xC0, 0xC0, 0xFC, 0xFE, 0xC3, 0xC3, 0x7E, 0x3C, // 6
        0xFF, 0xFF, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x60, 0x60, // 7
        0x3C, 0x7E, 0xC3, 0xC3, 0x7E, 0x7E, 0xC3, 0xC3, 0x7E, 0x3C, // 8
        0x3C, 0x7E, 0xC3, 0xC3, 0x7F, 0x3F, 0x03, 0x03, 0x3E, 0x7C, // 9
    ];

    /// Create and initialize a new CPU instance.
    pub fn new() -> Self {
        let mut memory = [0; 4 * 1024];
        memory[..80].clone_from_slice(&Self::DIGITS);
        memory[Chip8Core::LARGE_DIGIT_OFFSET..Chip8Core::LARGE_DIGIT_OFFSET + 100].clone_from_slice(&Self::LARGE_DIGITS);

        Self {
            instructions: Self::create_instructions(),
            registers: [0; 16],
            i_register: 0,
            memory,
            pc: Self::INITIAL_ADDR,
            stack: Vec::with_capacity(64),
            store_keypress: None,
            delay_timer: 0,
            sound_timer: 0,
        }
    }

    fn create_instructions() -> HashMap<&'static str, Instruction> {
        let instructions = vec![
            Instruction {
                name: "NOP",
                arg_masks: HashMap::new(),
                callback: Chip8Core::nop,
            },
            Instruction { // 00CN
                name: "SCD",
                arg_masks: HashMap::from([("N", Instruction::HEX_0)]),
                callback: Chip8Core::scd,
            },

            Instruction { // 00E0
                name: "CLS",
                arg_masks: HashMap::new(),
                callback: Chip8Core::cls,
            },
            Instruction { // 00EE
                name: "RET",
                arg_masks: HashMap::new(),
                callback: Chip8Core::ret,
            },
            Instruction { // 00FB
                name: "SCR",
                arg_masks: HashMap::new(),
                callback: Chip8Core::scr,
            },
            Instruction { // 00FC
                name: "SCL",
                arg_masks: HashMap::new(),
                callback: Chip8Core::scl,
            },
            Instruction { // 00FD
                name: "EXIT",
                arg_masks: HashMap::new(),
                callback: Chip8Core::exit,
            },
            Instruction { // 00FE
                name: "LORES",
                arg_masks: HashMap::new(),
                callback: Chip8Core::lores,
            },
            Instruction { // 00FF
                name: "HIRES",
                arg_masks: HashMap::new(),
                callback: Chip8Core::hires,
            },
            Instruction { // 1NNN
                name: "JMP",
                arg_masks: HashMap::from([("N", Instruction::HEX_012)]),
                callback: Chip8Core::jmp,
            },
            Instruction { // 2NNN
                name: "CALL",
                arg_masks: HashMap::from([("N", Instruction::HEX_012)]),
                callback: Chip8Core::call,
            },
            Instruction { // 3XNN
                name: "SKPEQ",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
                callback: Chip8Core::skpeq,
            },
            Instruction { // 4XNN
                name: "SKPNE",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
                callback: Chip8Core::skpne,
            },
            Instruction { // 5XY0
                name: "SKPEQR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::skpeqr,
            },
            Instruction { // 6XNN
                name: "MOV",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
                callback: Chip8Core::mov,
            },
            Instruction { // 7XNN
                name: "ADD",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
                callback: Chip8Core::add,
            },
            Instruction { // 8XY0
                name: "MOVR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::movr,
            },
            Instruction { // 8XY1
                name: "OR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::or,
            },
            Instruction { // 8XY2
                name: "AND",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::and,
            },
            Instruction { // 8XY3
                name: "XOR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::xor,
            },
            Instruction { // 8XY4
                name: "ADDR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::addr,
            },
            Instruction { // 8XY5
                name: "SUBR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::subr,
            },
            Instruction { // 8XY6
                name: "SHR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::shr,
            },
            Instruction { // 8XY7
                name: "RSUBR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::rsubr,
            },
            Instruction { // 8XYE
                name: "SHL",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::shl,
            },
            Instruction { // 9XY0
                name: "SKPNER",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
                callback: Chip8Core::skpner,
            },
            Instruction { // ANNN
                name: "MOVI",
                arg_masks: HashMap::from([("N", Instruction::HEX_012)]),
                callback: Chip8Core::movi,
            },
            Instruction { // BNNN
                name: "JMPR",
                arg_masks: HashMap::from([("N", Instruction::HEX_012)]),
                callback: Chip8Core::jmpr,
            },
            Instruction { // CXNN
                name: "RAND",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
                callback: Chip8Core::rand,
            },
            Instruction { // DXYN
                name: "DRAW",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1), ("N", Instruction::HEX_0)]),
                callback: Chip8Core::draw,
            },
            Instruction { // EX9E
                name: "SKPK",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::skpk,
            },
            Instruction { // EXA1
                name: "SKPNK",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::skpnk,
            },
            Instruction { // FX0A
                name: "KEY",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::key,
            },
            Instruction { // FX07
                name: "TIMR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::timr,
            },
            Instruction { // FX15
                name: "DELR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::delr,
            },
            Instruction { // FX29
                name: "DIGIT",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::digit,
            },
            Instruction {
                name: "LDIGIT",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::ldigit,
            },
            Instruction { // FX18
                name: "SNDR",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::sndr,
            },
            Instruction { // FX1E
                name: "ADDI",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::addi,
            },
            Instruction { // FX33
                name: "BCD",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::bcd,
            },
            Instruction { // FX55
                name: "SAVE",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::save,
            },
            Instruction { // FX65
                name: "LOAD",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::load,
            },
            Instruction { // FX75
                name: "SAVEF",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::savef,
            },
            Instruction { // FX85
                name: "LOADF",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::loadf,
            },
        ];

        instructions.into_iter().map(|i| (i.name, i)).collect()
    }

    fn instruction(&self, name: &str) -> &Instruction {
        self.instructions.get(name).unwrap()
    }

    fn fetch_byte(&mut self) -> u8 {
        let byte = self.memory[self.pc as usize];
        self.pc += 1;
        byte
    }

    /// Load a program into memory. Has no effect if the size of the program exceeds
    /// the available memory.
    pub fn load_program(&mut self, data: &[u8]) {
        // TODO: change return type to signal an error when program is too large.

        let addr = Self::INITIAL_ADDR as usize;
        let program_size = data.len();

        if program_size <= self.memory.len() - addr {
            self.memory[addr..addr + program_size].copy_from_slice(data);
        }
    }

    /// Fetches a raw 16-bit instruction from memory. Instructions are stored in big
    /// endian (most significant byte first).
    pub fn fetch_instruction(&mut self) -> u16 {
        let msb = self.fetch_byte() as u16;
        let lsb = self.fetch_byte() as u16;

        (msb << u8::BITS) | lsb
    }

    /// Decodes a raw 16-bit instruction. Note that the raw instruction is still
    /// required afterwards in order to obtain the instruction arguments.
    pub fn decode_instruction(&self, instruction: u16) -> &Instruction {
        let nop = self.instruction("NOP");

        match instruction & 0xF000 {
            0x0000 => match instruction & 0x00FF {
                0x00C0..=0x00CF => self.instruction("SCD"),
                0x00E0 => self.instruction("CLS"),
                0x00EE => self.instruction("RET"),
                0x00FB => self.instruction("SCR"),
                0x00FC => self.instruction("SCL"),
                0x00FD => self.instruction("EXIT"),
                0x00FE => self.instruction("LORES"),
                0x00FF => self.instruction("HIRES"),
                _ => nop,
            },
            0x1000 => self.instruction("JMP"),
            0x2000 => self.instruction("CALL"),
            0x3000 => self.instruction("SKPEQ"),
            0x4000 => self.instruction("SKPNE"),
            0x5000 => self.instruction("SKPEQR"),
            0x6000 => self.instruction("MOV"),
            0x7000 => self.instruction("ADD"),
            0x8000 => match instruction & 0x000F {
                0x0000 => self.instruction("MOVR"),
                0x0001 => self.instruction("OR"),
                0x0002 => self.instruction("AND"),
                0x0003 => self.instruction("XOR"),
                0x0004 => self.instruction("ADDR"),
                0x0005 => self.instruction("SUBR"),
                0x0006 => self.instruction("SHR"),
                0x0007 => self.instruction("RSUBR"),
                0x000E => self.instruction("SHL"),
                _ => nop,
            },
            0x9000 => self.instruction("SKPNER"),
            0xA000 => self.instruction("MOVI"),
            0xB000 => self.instruction("JMPR"),
            0xC000 => self.instruction("RAND"),
            0xD000 => self.instruction("DRAW"),
            0xE000 => match instruction & 0x00FF {
                0x009E => self.instruction("SKPK"),
                0x00A1 => self.instruction("SKPNK"),
                _ => nop,
            }
            0xF000 => match instruction & 0x00FF {
                0x000A => self.instruction("KEY"),
                0x0007 => self.instruction("TIMR"),
                0x0015 => self.instruction("DELR"),
                0x0018 => self.instruction("SNDR"),
                0x001E => self.instruction("ADDI"),
                0x0029 => self.instruction("DIGIT"),
                0x0030 => self.instruction("LDIGIT"),
                0x0033 => self.instruction("BCD"),
                0x0055 => self.instruction("SAVE"),
                0x0065 => self.instruction("LOAD"),
                0x0075 => self.instruction("SAVEF"),
                0x0085 => self.instruction("LOADF"),
                _ => nop,
            },
            _ => nop,
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}
