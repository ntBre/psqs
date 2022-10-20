use std;

use std::fmt::Display;

/// a histogram covering the range [min, max) with `N` bins
pub(crate) struct Histogram<const N: usize> {
    pub(crate) min: f64,
    pub(crate) cur_min: f64,
    pub(crate) cur_max: f64,
    pub(crate) total: f64,
    pub(crate) denom: f64,
    pub(crate) data: [usize; N],
}

impl<const N: usize> Histogram<N> {
    pub(crate) fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            denom: max - min,
            data: [0; N],
            cur_min: 0.0,
            cur_max: 0.0,
            total: 0.0,
        }
    }

    /// insert `val` into the appropriate bin in `self` and add it to the total.
    /// if `val` is greater than `self.max`, don't perform the insert but add it
    /// to the other statistics
    pub(crate) fn insert(&mut self, val: f64) {
        let idx = N as f64 * (val - self.min) / self.denom;
        if let Some(elt) = self.data.get_mut(idx.floor() as usize) {
            *elt += 1;
        }
        if val > self.cur_max {
            self.cur_max = val;
        }
        if val < self.cur_min {
            self.cur_min = val;
        }
        self.total += val;
    }

    /// return the count of elements in `self`
    pub(crate) fn count(&self) -> usize {
        self.data.iter().sum()
    }

    /// return the average of `self`
    pub(crate) fn average(&self) -> f64 {
        self.total / self.count() as f64
    }
}

impl<const N: usize> Display for Histogram<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bin_width = self.denom / N as f64;
        for (i, v) in self.data.iter().enumerate() {
            if *v > 0 {
                writeln!(f, "{:5.2}{:5}", i as f64 * bin_width, v)?;
            }
        }
        Ok(())
    }
}
