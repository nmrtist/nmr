//! Deterministic, overflow-resistant numerical reductions.

/// LAPACK-style scaled Euclidean norm over a canonical iterator order.
pub(crate) fn scaled_l2(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut scale = 0.0;
    let mut sumsq = 1.0;
    for value in values {
        let absolute = value.abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            let ratio = scale / absolute;
            sumsq = 1.0 + sumsq * ratio * ratio;
            scale = absolute;
        } else {
            let ratio = absolute / scale;
            sumsq += ratio * ratio;
        }
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * sumsq.sqrt()
    }
}

/// Neumaier compensated sum with dynamic magnitude scaling.
#[allow(dead_code)] // Shared with the bounded optimizers and reconstruction profiles.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ScaledNeumaier {
    scale: f64,
    sum: f64,
    compensation: f64,
}

impl ScaledNeumaier {
    #[allow(dead_code)]
    /// Adds one finite value in canonical order.
    pub(crate) fn add(&mut self, value: f64) {
        if value == 0.0 {
            return;
        }
        let absolute = value.abs();
        if self.scale == 0.0 {
            self.scale = absolute;
            self.sum = value / absolute;
            return;
        }
        if absolute > self.scale {
            let ratio = self.scale / absolute;
            self.sum *= ratio;
            self.compensation *= ratio;
            self.scale = absolute;
        }
        let normalized = value / self.scale;
        let next = self.sum + normalized;
        if self.sum.abs() >= normalized.abs() {
            self.compensation += (self.sum - next) + normalized;
        } else {
            self.compensation += (normalized - next) + self.sum;
        }
        self.sum = next;
    }

    /// Returns the compensated sum.
    #[allow(dead_code)]
    pub(crate) fn total(self) -> f64 {
        self.scale * (self.sum + self.compensation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_l2_avoids_intermediate_overflow() {
        let norm = scaled_l2([3e200, 4e200]);
        assert!((norm / 5e200 - 1.0).abs() < 1e-15);
    }

    #[test]
    fn scaled_neumaier_retains_small_residual() {
        let mut sum = ScaledNeumaier::default();
        for value in [1e100, 1.0, -1e100] {
            sum.add(value);
        }
        assert_eq!(sum.total(), 1.0);
    }
}
