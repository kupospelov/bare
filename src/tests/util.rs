use std::time::Instant;

pub fn bench(name: &str, mut operation: impl FnMut()) {
    const N: u32 = 100_000;
    for _ in 0..1_000 {
        operation();
    }
    let start = Instant::now();
    for _ in 0..N {
        operation();
    }
    let ns = start.elapsed().as_nanos() as f64 / N as f64;
    eprintln!("{name}: {ns:.2}ns");
}
