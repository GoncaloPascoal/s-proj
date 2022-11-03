
use libretro_rs::{libretro_core, RetroCore, RetroEnvironment, RetroGame,
    RetroLoadGameResult, RetroRuntime, RetroSystemInfo};

struct Chip8Core;

impl RetroCore for Chip8Core {
    fn init(env: &RetroEnvironment) -> Self {
        Chip8Core {}
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
