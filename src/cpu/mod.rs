
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
}

impl Cpu {
    const INITIAL_ADDR: u16 = 0x200;

    /// Create and initialize a new CPU instance.
    pub fn new() -> Self {
        Self {
            instructions: Self::create_instructions(),
            registers: [0; 16],
            i_register: 0,
            memory: [0; 4 * 1024],
            pc: Self::INITIAL_ADDR,
            stack: Vec::with_capacity(64),
        }
    }

    fn create_instructions() -> HashMap<&'static str, Instruction> {
        let instructions = vec![
            Instruction {
                name: "NOP",
                arg_masks: HashMap::new(),
                callback: Chip8Core::nop,
            },
            Instruction { // 00E0
                name: "CLS",
                arg_masks: HashMap::new(),
                callback: Chip8Core::cls,
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
            Instruction { // DXYN
                name: "DRAW",
                arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1), ("N", Instruction::HEX_0)]),
                callback: Chip8Core::draw,
            },
            Instruction { // FX1E
                name: "ADDI",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::addi,
            },
            Instruction { // FX55
                name: "SAVE",
                arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
                callback: Chip8Core::save,
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

        return (msb << u8::BITS) | lsb;
    }

    /// Decodes a raw 16-bit instruction. Note that the raw instruction is still
    /// required afterwards in order to obtain the instruction arguments.
    pub fn decode_instruction(&self, instruction: u16) -> &Instruction {
        let nop = self.instruction("NOP");

        match instruction & 0xF000 {
            0x0000 => match instruction & 0x00FF {
                0x00E0 => self.instruction("CLS"),
                // 0x00EE => self.instruction("RET"),
                _ => nop,
            },
            0x1000 => self.instruction("JMP"),
            0x2000 => self.instruction("CALL"),
            0x3000 => self.instruction("SKPEQ"),
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
            }
            0xA000 => self.instruction("MOVI"),
            0xB000 => self.instruction("JMPR"),
            0xD000 => self.instruction("DRAW"),
            0xF000 => match instruction & 0x00FF {
                0x001E => self.instruction("ADDI"),
                0x0055 => self.instruction("SAVE"),
                _ => nop,
            },
            _ => nop,
        }
    }
}