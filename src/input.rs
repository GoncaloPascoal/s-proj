
use strum_macros::EnumIter;

#[derive(EnumIter)]
pub enum Chip8Key {
    Num1 = 49,
    Num2 = 50,
    Num3 = 51,
    Num4 = 52,
    Q    = 113,
    W    = 119,
    E    = 101,
    R    = 114,
    A    = 97,
    S    = 115,
    D    = 100,
    F    = 102,
    Z    = 122,
    X    = 120,
    C    = 99,
    V    = 118,
}
