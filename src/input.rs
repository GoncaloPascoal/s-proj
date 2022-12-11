
use strum_macros::EnumIter;

#[derive(EnumIter)]
pub enum Chip8Key {
    X    = 120,
    Num1 = 49,
    Num2 = 50,
    Num3 = 51,
    Q    = 113,
    W    = 119,
    E    = 101,
    A    = 97,
    S    = 115,
    D    = 100,
    Z    = 122,
    C    = 99,
    Num4 = 52,
    R    = 114,
    F    = 102,
    V    = 118,
}
