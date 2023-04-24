pub fn halton(i: u32, b: u32) -> f32 {
    let mut f = 1.0;
    let mut r = 0.0;
    let mut i = i;
    while i > 0 {
        f /= b as f32;
        r += f * (i % b) as f32;
        i /= b;
    }
    r
}

pub fn get_jitter(jitter_index: u32, resolution: &[f32; 2]) -> [f32; 2] {
    let jitter = [
        2.0 * halton(jitter_index, 2) - 1.0,
        2.0 * halton(jitter_index, 3) - 1.0,
    ];
    
    [
        jitter[0] / resolution[0],
        jitter[1] / resolution[1],
    ]
}