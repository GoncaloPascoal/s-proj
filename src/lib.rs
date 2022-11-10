
use std::collections::HashMap;

use libretro_rs::{libretro_core, RetroCore, RetroEnvironment, RetroGame,
    RetroLoadGameResult, RetroRuntime, RetroSystemInfo};

struct Instruction {
    arg_masks: HashMap<&'static str, u16>,
    callback: fn(&mut Chip8Core, HashMap<&'static str, u16>),
}

impl Instruction {
    // Useful constants for specifying bit masks
    const HEX_0: u16 = 0x000F;
    const HEX_1: u16 = 0x00F0;
    const HEX_2: u16 = 0x0F00;
    const HEX_3: u16 = 0xF000;
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

    fn todo(core: &mut Chip8Core, args: HashMap<&'static str, u16>) {
        todo!()
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

        instructions.insert(0xA000, Instruction {
            arg_masks: HashMap::from([("N", Instruction::HEX_012)]),
            callback: Instruction::todo,
        });

        instructions.insert(0x8000, Instruction {
            arg_masks: HashMap::from([("X", Instruction::HEX_2), ("Y", Instruction::HEX_1)]),
            callback: Instruction::todo,
        });

        instructions
    }
}

struct Chip8Core {
    cpu: Cpu,
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
