use std::cmp::Ordering;

use spatial::Point2D;

/// A closed interval; essentially a floating-point `RangeInclusive` with some convenience methods.
#[derive(Clone)]
pub struct Interval {
    pub start: f64,
    pub end: f64,
    pub step: f64,
}

impl Iterator for Interval {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start > self.end {
            None
        } else {
            let start = self.start;
            self.start += self.step;
            Some(start)
        }
    }
}

/// An `f64` that implements `Ord`, when we don't care about NaNs. Specifically, `OrdFloat` is
/// ordered as `f64`, but treats all NaNs as being equal and less than any other value.
#[derive(Clone, Copy, Debug)]
pub struct OrdFloat(pub f64);

impl OrdFloat {
    pub fn new(x: f64) -> Option<OrdFloat> {
        if !x.is_nan() {
            Some(OrdFloat(x))
        } else {
            None
        }
    }
}

impl PartialEq for OrdFloat {
    fn eq(&self, other: &OrdFloat) -> bool {
        if !self.0.is_nan() || !other.0.is_nan() {
            self.0.eq(&other.0)
        } else {
            // All NaNs are considered equal.
            true
        }
    }
}

impl Eq for OrdFloat {}

impl PartialOrd for OrdFloat {
    fn partial_cmp(&self, other: &OrdFloat) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrdFloat {
    fn cmp(&self, other: &OrdFloat) -> Ordering {
        match (self.0.is_nan(), other.0.is_nan()) {
            // Non-NaNs are all comparable.
            (false, false) => self.0.partial_cmp(&other.0).unwrap(),
            // Otherwise any non-NaN is larger, or two NaNs are equal.
            (x, y) => y.cmp(&x),
        }
    }
}

/// A parametric equation ℝ → ℝ × ℝ.
pub struct Equation<'a> {
    pub function: Box<dyn 'a + Fn(f64) -> Point2D>,
}

impl<'a> Equation<'a> {
    /// Sample the equation over an interval.
    pub fn sample(&self, interval: &Interval) -> Vec<Point2D> {
        interval.clone().map(|t| (self.function)(t)).collect()
    }

    /// Return a new equation representing the normal at the given `t`.
    pub fn normal(&self, t: f64) -> Equation {
        let (mx, my) = (self.function)(t);
        let (dx, dy) = self.derivative(t);

        Equation {
            function: box move |s| {
                (mx - s * dy, my + s * dx)
            }
        }
    }

    // pub fn gradient(&self, t: f64) -> Angle {
    //     let (dx, dy) = self.derivative(t);
    //     Angle::new(dy.atan2(dx))
    // }

    /// Return the gradient vector at the given `t`: i.e. the value of the derivative at `t`.
    pub fn derivative(&self, t: f64) -> Point2D {
        // The function approximates the derivative using `(f(t + H) - f(t - H)) / 2 * H`.
        const H: f64 = 0.1;

        let f = &self.function;
        let (fp, fm) = (f(t + H), f(t - H));
        let d = 2.0 * H;
        ((fp.0 - fm.0) / d, (fp.1 - fm.1) / d)
    }
}

// pub struct Region {
//     pub origin: (f64, f64),
// }

pub struct View {
    pub cols: u16,
    pub rows: u16,
    pub size: f64,
    pub x: f64,
    pub y: f64,
}

impl View {
    pub fn project(&self, (x, y): (f64, f64)) -> Option<(usize, usize)> {
        let (x, y) = (((x - self.x) / self.size).round(), ((y - self.y) / self.size).round());
        if x >= 0.0 && y >= 0.0 {
            let (x, y) = (x as usize, y as usize);
            if x < self.cols as usize && y < self.rows as usize {
                return Some((x, y));
            }
        }
        None
    }
}
