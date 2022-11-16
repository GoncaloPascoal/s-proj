
use super::*;

/// Prints the disassembled program to standard output, including its instructions,
/// respective arguments and memory locations.
pub fn disassemble(data: &[u8]) {
    let mut cpu = Cpu::new();
    cpu.load_program(data);

    for _ in 0..data.len() / 2 {
        let addr = cpu.pc;
        let raw = cpu.fetch_instruction();
        let instruction = cpu.decode_instruction(raw);

        print!("0x{:X} ({}) | 0x{:04X} | {} [", addr, addr, raw, instruction.name);

        let mut args_str = Vec::new();
        for arg in instruction.args(raw) {
            args_str.push(format!("{} = 0x{:X}", arg.0, arg.1));
        }

        println!("{}]", args_str.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disassemble_test() {
        let data = [0x84, 0xF2, 0x8E, 0x10, 0xA4, 0x53];
        disassemble(data.as_slice());
    }
}
