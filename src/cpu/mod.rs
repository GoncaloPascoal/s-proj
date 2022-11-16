
use std::collections::HashMap;
use crate::Chip8Core;

pub struct Instruction {
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
        }
    }

    fn create_instructions() -> HashMap<&'static str, Instruction> {
        let mut instructions = HashMap::new();

        instructions.insert("MOV", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
            callback: Chip8Core::mov,
        });

        instructions.insert("ADD", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
            callback: Chip8Core::add,
        });

        instructions.insert("MOVR", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::movr,
        });

        instructions.insert("OR", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::or,
        });

        instructions.insert("AND", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::and,
        });

        instructions.insert("XOR", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::xor,
        });

        instructions.insert("ADDR", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::addr,
        });

        instructions.insert("SUBR", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::subr,
        });

        instructions.insert("SHR", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::shr,
        });

        instructions.insert("RSUBR", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::rsubr,
        });

        instructions.insert("SHL", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::shl,
        });

        instructions.insert("MOVI", Instruction {
            arg_masks: HashMap::from([("N", Instruction::HEX_012)]),
            callback: Chip8Core::movi,
        });

        instructions.insert("ADDI", Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
            callback: Chip8Core::addi,
        });

        instructions
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
        match instruction & 0xF000 {
            0x6000 => self.instruction("MOV"),
            0x7000 => self.instruction("ADD"),
            0x8000 => match instruction & 0x000F {
                0x0001 => self.instruction("OR"),
                0x0002 => self.instruction("AND"),
                0x0003 => self.instruction("XOR"),
                0x0004 => self.instruction("ADDR"),
                0x0005 => self.instruction("SUBR"),
                0x0006 => self.instruction("SHR"),
                0x0007 => self.instruction("RSUBR"),
                0x000E => self.instruction("SHL"),
                _ => unreachable!()
            }
            0xA000 => self.instruction("MOVI"),
            0xF000 => match instruction & 0x00FF {
                0x001E => self.instruction("ADDI"),
                _ => unreachable!()
            },
            _ => unreachable!(),
        }
    }
}
