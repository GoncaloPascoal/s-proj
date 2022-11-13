
use std::collections::HashMap;

use libretro_rs::{libretro_core, RetroCore, RetroEnvironment, RetroGame,
    RetroLoadGameResult, RetroRuntime, RetroSystemInfo};

struct Instruction {
    name: &'static str,
    arg_masks: HashMap<&'static str, u16>,
    callback: fn(&mut Chip8Core, HashMap<&'static str, u16>),
}

impl Instruction {
    // Useful constants for specifying bit masks
    const HEX_0: u16 = 0x000F;
    const HEX_1: u16 = 0x00F0;
    const HEX_2: u16 = 0x0F00;
    const HEX_01: u16 = Instruction::HEX_0 | Instruction::HEX_1;    // 0x00FF
    const HEX_12: u16 = Instruction::HEX_1 | Instruction::HEX_2;    // 0x0FF0
    const HEX_012: u16 = Instruction::HEX_0 | Instruction::HEX_12;  // 0x0FFF

    // Get a single argument via mask
    fn arg(&self, instruction: u16, id: &str) -> u16 {
        let mask = self.arg_masks.get(id).unwrap();
        (instruction & mask) >> mask.trailing_zeros()
    }

    fn args(&self, instruction: u16) -> HashMap<&'static str, u16> {
        self.arg_masks.iter().map(|(&k, _)| (k, self.arg(instruction, k))).collect()
    }
}

struct Cpu {
    instructions: HashMap<u16, Instruction>,
    registers: [u8; 16],
    i_register: u16,
    memory: [u8; 4 * 1024], // 4 KiB RAM
}

impl Cpu {
    fn new() -> Cpu {
        Cpu {
            instructions: Cpu::create_instructions(),
            registers: [0; 16],
            i_register: 0,
            memory: [0; 4 * 1024],
        }
    }

    fn create_instructions() -> HashMap<u16, Instruction> {
        let mut instructions = HashMap::new();

        instructions.insert(0x6000, Instruction {
            name: "MOV",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
            callback: Chip8Core::mov,
        });

        instructions.insert(0x7000, Instruction {
            name: "ADD",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("N", Instruction::HEX_01)]),
            callback: Chip8Core::add,
        });

        instructions.insert(0x8000, Instruction {
            name: "MOVR",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::movr,
        });

        instructions.insert(0x8001, Instruction {
            name: "OR",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::or,
        });

        instructions.insert(0x8002, Instruction {
            name: "AND",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::and,
        });

        instructions.insert(0x8003, Instruction {
            name: "XOR",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::xor,
        });

        instructions.insert(0x8004, Instruction {
            name: "ADDR",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::addr,
        });

        instructions.insert(0x8005, Instruction {
            name: "SUBR",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::subr,
        });

        instructions.insert(0x8006, Instruction {
            name: "SHR",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::shr,
        });

        instructions.insert(0x8007, Instruction {
            name: "RSUBR",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::rsubr,
        });

        instructions.insert(0x800E, Instruction {
            name: "SHL",
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Chip8Core::shl,
        });

        instructions.insert(0xA000, Instruction {
            name: "MOVI",
            arg_masks: HashMap::from([("N", Instruction::HEX_012)]),
            callback: Chip8Core::movi,
        });

        instructions.insert(0xF01E, Instruction {
            name: "ADDI",
            arg_masks: HashMap::from([("X", Instruction::HEX_2)]),
            callback: Chip8Core::addi,
        });

        instructions
    }
}

struct Chip8Core {
    cpu: Cpu,
}

impl Chip8Core {
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
}

impl RetroCore for Chip8Core {
    fn init(_env: &RetroEnvironment) -> Self {
        Chip8Core { cpu: Cpu::new() }
    }

    fn get_system_info() -> RetroSystemInfo {
        RetroSystemInfo::new("CHIP-8 Emulator", "0.1.0")
    }

    fn reset(&mut self, env: &RetroEnvironment) {

    }

    fn run(&mut self, env: &RetroEnvironment, runtime: &RetroRuntime) {

    }

    fn load_game(&mut self, env: &RetroEnvironment, game: RetroGame) -> RetroLoadGameResult {
        RetroLoadGameResult::Failure
    }
}

libretro_core!(Chip8Core);
