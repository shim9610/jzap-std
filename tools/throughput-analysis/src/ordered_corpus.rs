//! Shared deterministic ordered-container inputs; generation is outside timing.
pub struct Case {
    pub keys: Vec<u64>,
    pub pivot: u64,
}
fn random(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
pub fn corpus(mut seed: u64, small: bool) -> Vec<Case> {
    (0..64)
        .map(|c| {
            let n = match c % 16 {
                0 => 0,
                1 => 1,
                2 => {
                    if small {
                        32
                    } else {
                        4096
                    }
                }
                _ => {
                    let e = random(&mut seed) % if small { 6 } else { 13 };
                    (random(&mut seed) as usize % ((1usize << e).max(1))) + 1
                }
            };
            let mut keys = (0..n as u64).map(|k| k * 2 + 2).collect::<Vec<_>>();
            for i in (1..n).rev() {
                let j = random(&mut seed) as usize % (i + 1);
                keys.swap(i, j);
            }
            Case {
                keys,
                pivot: random(&mut seed) % ((n as u64 * 2 + 1).max(1)),
            }
        })
        .collect()
}
