pub const fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

pub const fn pad_4(x: usize) -> usize {
    (x + 3) & !3
}
