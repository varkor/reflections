use std::cmp::Ordering;
// use std::cmp::Reverse;
// use std::collections::BinaryHeap;
// use std::f64::consts::PI;
// use std::fmt::Debug;
// use std::ops::RangeInclusive;

use spade::BoundingRect;
use spade::PointN;
use spade::SpatialObject;

/// A simple key-value pair. Traits are implemented solely on the key.
// #[derive(Clone, Copy)]
// pub struct KeyValue<K, V>(pub K, pub V);

// impl<K: PartialEq, V> PartialEq for KeyValue<K, V> {
//     fn eq(&self, other: &KeyValue<K, V>) -> bool {
//         self.0.eq(&other.0)
//     }
// }

// impl<K: Eq, V> Eq for KeyValue<K, V> {}

// impl<K: PartialOrd, V> PartialOrd for KeyValue<K, V> {
//     fn partial_cmp(&self, other: &KeyValue<K, V>) -> Option<Ordering> {
//         self.0.partial_cmp(&other.0)
//     }
// }

// impl<K: Ord, V> Ord for KeyValue<K, V> {
//     fn cmp(&self, other: &KeyValue<K, V>) -> Ordering {
//         self.0.cmp(&other.0)
//     }
// }

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

/// A metric: a function defining a distance between two objects.
// pub trait Metric {
//     type Output: Ord;

//     fn distance(&self, &Self) -> Self::Output;
// }

// impl Metric for () {
//     type Output = ();

//     fn distance(&self, _: &Self) -> Self::Output {
//         ()
//     }
// }

/// An angle in radians. Guaranteed to be in the range [0, 2π).
// #[derive(Clone, Copy)]
// pub struct Angle(f64);

// const TAU: f64 = 2.0 * PI;

// impl Angle {
//     pub fn new(a: f64) -> Self {
//         Self(a.mod_euc(TAU))
//     }
// }

// impl Metric for Angle {
//     type Output = OrdFloat;

//     fn distance(&self, other: &Self) -> Self::Output {
//         OrdFloat(((self.0 - other.0 + PI).mod_euc(TAU) - PI).abs())
//     }
// }

// impl Metric for f64 {
//     type Output = OrdFloat;

//     fn distance(&self, other: &Self) -> Self::Output {
//         OrdFloat(self - other)
//     }
// }

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
    pub fn sample(&self, interval: &Interval) -> Vec<Point2D> {
        interval.clone().map(|t| (self.function)(t)).collect()
    }

    pub fn normal(&self, t: f64) -> Equation {
        let (mx, my) = (self.function)(t);
        let (dx, dy) = self.derivative(t);

        Equation { function: Box::new(move |s| {
            (mx - s * dy, my + s * dx)
        }) }
    }

    // pub fn gradient(&self, t: f64) -> Angle {
    //     let (dx, dy) = self.derivative(t);
    //     Angle::new(dy.atan2(dx))
    // }

    pub fn derivative(&self, t: f64) -> Point2D {
        let f = &self.function;
        let h = 0.1;
        let (fp, fm) = (f(t + h), f(t - h));
        let d = 2.0 * h;
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

// FIXME: replace this with an iterator
// pub fn adaptive_sample<K: Clone + Metric, V: Clone, F: Fn(f64) -> KeyValue<K, V>>(
//     f: F,
//     range: &RangeInclusive<f64>,
//     samples: u64,
// ) -> Vec<V>
//     where <K as Metric>::Output: Ord + Debug,
// {
//     // println!("adaptive_sample");
//     assert!(samples >= 2);

//     let mut pq = BinaryHeap::new();

//     let evaled_pair = |t: f64| -> (f64, KeyValue<K, V>) {
//         (t, f(t))
//     };

//     let mut i = 0;

//     let mut add_segment = |
//         pq: &mut BinaryHeap<KeyValue<(<K as Metric>::Output, Reverse<i64>), ((f64, KeyValue<K, V>), (f64, KeyValue<K, V>))>>,
//         low: (f64, KeyValue<K, V>),
//         high: (f64, KeyValue<K, V>),
//     | {
//         pq.push(KeyValue(((&(high.1).0).distance(&(low.1).0), Reverse(i)), (low, high)));
//         i += 1;
//     };

//     let (t_min, t_max) = range.clone().into_inner();
//     let (min, max) = (evaled_pair(t_min), evaled_pair(t_max));
//     let mut ts = vec![(min.1).1.clone(), (max.1).1.clone()];

//     add_segment(&mut pq, min, max);

//     while (ts.len() as u64) < samples {
//         // Get the segment with the largest distance.
//         let KeyValue(distance, (low, high)) = pq.pop().unwrap();
//         // Get the midpoint of the segment.
//         let mid = evaled_pair(low.0 / 2.0 + high.0 / 2.0);
//         println!("{:?} {:?} {:?} {:?}", distance, low.0, high.0, mid.0);
//         ts.push((mid.1).1.clone());
//         add_segment(&mut pq, low, mid.clone());
//         add_segment(&mut pq, mid, high);
//     }

//     ts
// }

pub type Point2D = (f64, f64);

// impl Metric for Point2D {
//     type Output = OrdFloat;

//     fn distance(&self, other: &Self) -> Self::Output {
//         OrdFloat((self.0 - other.0).powf(2.0) + (self.1 - other.1).powf(2.0))
//     }
// }

/// A `SpatialObject` that also carries data. Methods are simply forwarded to the `SpatialObject`.
#[derive(Clone)]
pub struct SpatialObjectWithData<S: SpatialObject, T>(pub S, pub T);

impl<S: SpatialObject, T> SpatialObject for SpatialObjectWithData<S, T> {
    type Point = <S as SpatialObject>::Point;

    fn mbr(&self) -> BoundingRect<Self::Point> {
        self.0.mbr()
    }

    fn distance2(&self, point: &Self::Point) -> <Self::Point as PointN>::Scalar {
        self.0.distance2(point)
    }
}
