//! PCG RXS-M-XS with a separate stream for each (seed, pixel, sample).
//! The Metal implementation uses the same wrapping integer operations and 24-bit floats.

pub struct Sampler(u32);

impl Sampler {
    pub fn new(seed: u32, pixel: u32, sample: u32) -> Self {
        Self(mix(seed
            ^ mix(pixel)
            ^ mix(sample.wrapping_add(0x9e3779b9))))
    }

    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(747796405).wrapping_add(2891336453);
        let word = ((self.0 >> ((self.0 >> 28) + 4)) ^ self.0).wrapping_mul(277803737);
        f64::from(((word >> 22) ^ word) >> 8) / 16777216.0
    }
}

fn mix(mut value: u32) -> u32 {
    value = (value ^ (value >> 16)).wrapping_mul(0x7feb352d);
    value = (value ^ (value >> 15)).wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}
