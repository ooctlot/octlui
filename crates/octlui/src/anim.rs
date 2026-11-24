//! Spring physics.

/// Spring configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringParams {
    /// Pull towards the target (higher = snappier).
    pub stiffness: f32,
    /// Velocity damping. Critical damping is `2 * sqrt(stiffness * mass)`.
    pub damping: f32,
    pub mass: f32,
    /// Distance/velocity below which the spring is considered at rest.
    pub precision: f32,
}
impl SpringParams {
    /// Critically damped spring: fastest approach without overshoot.
    pub fn critical(stiffness: f32) -> Self {
        Self { stiffness, damping: 2.0 * stiffness.sqrt(), mass: 1.0, precision: 0.01 }
    }
}
impl Default for SpringParams {
    fn default() -> Self {
        Self::critical(300.0)
    }
}

/// State of an N-dimensional spring.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpringState<const N: usize> {
    pub x: [f32; N],
    pub v: [f32; N],
}
impl<const N: usize> SpringState<N> {
    pub fn at(x: [f32; N]) -> Self {
        Self { x, v: [0.0; N] }
    }
    /// Advances by `dt` seconds. Returns `true` while still moving.
    pub fn step(&mut self, target: [f32; N], p: &SpringParams, dt: f32) -> bool {
        // Fixed sub-steps keep the integration stable for stiff springs and large frame times.
        const SUB: f32 = 1.0 / 240.0;
        let mut left = dt.min(0.25);
        while left > 0.0 {
            let h = left.min(SUB);
            for i in 0..N {
                let f = -p.stiffness * (self.x[i] - target[i]) - p.damping * self.v[i];
                self.v[i] += f / p.mass * h;
                self.x[i] += self.v[i] * h;
            }
            left -= h;
        }
        let mut moving = false;
        for i in 0..N {
            if (self.x[i] - target[i]).abs() > p.precision || self.v[i].abs() > p.precision {
                moving = true;
            }
        }
        if !moving {
            self.x = target;
            self.v = [0.0; N];
        }
        moving
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converges() {
        let mut s = SpringState::<1>::at([0.0]);
        let p = SpringParams::default();
        let mut frames = 0;
        while s.step([100.0], &p, 1.0 / 60.0) {
            frames += 1;
            assert!(frames < 600, "did not settle");
        }
        assert_eq!(s.x, [100.0]);
    }
}
