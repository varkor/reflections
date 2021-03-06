use std::cmp::Ordering;

use crate::spatial::Point2D;

/// A closed interval; essentially a floating-point `RangeInclusive` with some convenience methods.
#[derive(Clone)]
pub struct Interval {
    pub start: f64,
    pub end: f64,
    pub step: f64,
}

impl Interval {
    pub fn endpoints(start: f64, end: f64) -> Self {
        Interval { start, end, step: end - start }
    }
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

impl From<OrdFloat> for f64 {
    fn from(x: OrdFloat) -> f64 {
        x.0
    }
}

/// A parametric equation ℝ × ℝ → ℝ × ℝ.
pub struct Equation<'a, I> {
    pub function: Box<dyn 'a + Fn(I) -> Point2D>,
}

impl<'a> Equation<'a, f64> {
    /// Sample the equation over an interval.
    pub fn sample(&self, interval: &Interval) -> Vec<Point2D> {
        interval.clone().map(|t| (self.function)(t)).collect()
    }

    /// Return a new equation representing the normal at the given `t`.
    pub fn normal(&self, t: f64) -> Equation<'_, f64> {
        let [mx, my] = (self.function)(t).into_inner();
        let [dx, dy] = self.derivative(t).normalise().into_inner();

        Equation {
            function: box move |s| {
                Point2D::new([mx - s * dy, my + s * dx])
            }
        }
    }

    /// Return the gradient vector at the given `t`: i.e. the value of the derivative at `t`.
    pub fn derivative(&self, t: f64) -> Point2D {
        // The function approximates the derivative using `(f(t + H) - f(t - H)) / 2 * H`.
        const H: f64 = 0.1;

        let f = &self.function;
        let (fp, fm) = (f(t + H), f(t - H));
        let d = 2.0 * H;
        (fp - fm) / Point2D::diag(d)
    }
}

/// A view contains information both about the region being displayed (in cartesian coördinates), as
/// well as the size (in pixels) of the canvas on which it is displayed.
///
/// The struct `View` mirrors the JavaScript class `View` and should be kept in sync.
#[derive(Deserialize)]
pub struct View {
    /// The dimensions of the view canvas in pixels.
    pub width: u16,
    pub height: u16,
    /// The origin (centre) of the region in cartesian coördinates.
    pub origin: Point2D,
    /// The scale factor (in powers of 2) of the displayed region. E.g. `scale = 0` means a 1:1
    /// aspect ratio; `scale = 1` means zooming in 2x, etc.
    pub scale: f64,
}

impl View {
    /// Returns the width and height of the region in cartesian distances.
    pub fn size(&self) -> Point2D {
        let factor = 2.0f64.powf(self.scale);
        Point2D::new([self.width as f64, self.height as f64]) * Point2D::diag(factor)
    }

    /// Takes a point in cartesian coördinates and returns the corresponding pixel coördinates of
    /// the point in the given region.
    pub fn project(&self, p: Point2D, region: [usize; 2]) -> Option<[usize; 2]> {
        if p.is_nan() {
            return None;
        }

        let q = p - (self.origin - self.size() / Point2D::diag(2.0));
        if q >= Point2D::zero() && q < self.size() {
            let region = Point2D::new([region[0] as f64, region[1] as f64]);
            let [x, y] = (q * region / self.size()).into_inner();
            Some([x as usize, y as usize])
        } else {
            None
        }
    }
}
