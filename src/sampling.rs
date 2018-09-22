use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::f64::consts::PI;
use std::fmt::Debug;
use std::ops::RangeInclusive;

use approximation::OrdFloat;
use spatial::Point2D;

/// A simple key-value pair. Traits are implemented solely on the key.
#[derive(Clone, Copy)]
pub struct KeyValue<K, V>(pub K, pub V);

impl<K: PartialEq, V> PartialEq for KeyValue<K, V> {
    fn eq(&self, other: &KeyValue<K, V>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K: Eq, V> Eq for KeyValue<K, V> {}

impl<K: PartialOrd, V> PartialOrd for KeyValue<K, V> {
    fn partial_cmp(&self, other: &KeyValue<K, V>) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<K: Ord, V> Ord for KeyValue<K, V> {
    fn cmp(&self, other: &KeyValue<K, V>) -> Ordering {
        self.0.cmp(&other.0)
    }
}

/// A metric: a function defining a distance between two objects.
pub trait Metric {
    type Output: Ord;

    fn distance(&self, &Self) -> Self::Output;
}

impl Metric for () {
    type Output = ();

    fn distance(&self, _: &Self) -> Self::Output {
        ()
    }
}

/// An angle in radians. Guaranteed to be in the range [0, 2π).
#[derive(Clone, Copy)]
pub struct Angle(f64);

const TAU: f64 = 2.0 * PI;

impl Angle {
    pub fn new(a: f64) -> Self {
        Self(a.mod_euc(TAU))
    }
}

impl Metric for Angle {
    type Output = OrdFloat;

    fn distance(&self, other: &Self) -> Self::Output {
        OrdFloat(((self.0 - other.0 + PI).mod_euc(TAU) - PI).abs())
    }
}

impl Metric for f64 {
    type Output = OrdFloat;

    fn distance(&self, other: &Self) -> Self::Output {
        OrdFloat(self - other)
    }
}

impl Metric for Point2D {
    type Output = OrdFloat;

    fn distance(&self, other: &Self) -> Self::Output {
        OrdFloat((*self - *other).map(|x| x.powf(2.0)).sum())
    }
}

/// Sample a range according to a metric, bisecting subranges of the greatest distance to produce
/// evenly-spaced samples.
pub fn adaptive_sample<K: Clone + Metric, V: Clone, F: Fn(f64) -> KeyValue<K, V>>(
    f: F,
    range: RangeInclusive<f64>,
    samples: u64,
) -> Vec<V>
    where <K as Metric>::Output: Ord + Debug,
{
    assert!(samples >= 2);

    // The priority queue containing the segments left to be subdivided.
    let mut pq = BinaryHeap::new();
    // We need a tie-break. A simple ascending accumulator suffices to balance the priorities.
    let mut i = 0;

    let mut add_segment = |
        pq: &mut BinaryHeap<_>,
        low: (f64, KeyValue<K, V>),
        high: (f64, KeyValue<K, V>),
    | {
        pq.push(KeyValue(((&(high.1).0).distance(&(low.1).0), Reverse(i)), (low, high)));
        i += 1;
    };

    let eval_pair = |t: f64| -> (f64, KeyValue<K, V>) { (t, f(t)) };

    let (t_min, t_max) = range.into_inner();
    let (min, max) = (eval_pair(t_min), eval_pair(t_max));
    // `ts` contains an list of the values of `t` to sample to produce an evenly-spaced sampling.
    // We must at least sample the first and last points.
    let mut ts = vec![(min.1).1.clone(), (max.1).1.clone()];

    // Start off by considering the entire range.
    add_segment(&mut pq, min, max);

    while (ts.len() as u64) < samples {
        // Get the segment with the largest distance.
        let KeyValue(_, (low, high)) = pq.pop().unwrap();
        // Get the midpoint of the segment.
        let mid = eval_pair(low.0 / 2.0 + high.0 / 2.0);
        ts.push((mid.1).1.clone());
        add_segment(&mut pq, low, mid.clone());
        add_segment(&mut pq, mid, high);
    }

    ts
}
