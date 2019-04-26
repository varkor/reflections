use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Sub};

use num_traits::{sign::Signed, bounds::Bounded};
use rstar::{AABB, Envelope, Point, PointDistance, primitives::Line, RTreeObject};

use crate::approximation::OrdFloat;

/// A cartesian point with some helper methods.
#[derive(Clone, Copy, Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct Pair<T>([T; 2]);

impl<T: Copy> Pair<T> {
    pub fn new(p: [T; 2]) -> Self {
        Self(p)
    }

    pub fn diag(d: T) -> Self {
        Self([d; 2])
    }

    pub fn into_inner(self) -> [T; 2] {
        self.0
    }

    #[inline]
    pub fn x(&self) -> T {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> T {
        self.0[1]
    }

    pub fn map<S>(self, f: impl Fn(T) -> S) -> Pair<S> {
        Pair([f(self.x()), f(self.y())])
    }
}

impl<T: Copy + Mul<Output = T> + Add<Output = T>> Pair<T> {
    pub fn length_2(&self) -> T {
        self.0[0] * self.0[0] + self.0[1] * self.0[1]
    }
}

impl<T: Copy + Debug + PartialOrd + Signed + Bounded> Point for Pair<T> {
    type Scalar = T;
    const DIMENSIONS: usize = 2;

    fn generate(generator: impl Fn(usize) -> Self::Scalar) -> Self {
        Pair([generator(0), generator(1)])
    }

    fn nth(&self, index: usize) -> Self::Scalar {
        self.0[index]
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self.0[index]
    }
}

impl<T: Copy + PartialOrd> PartialOrd for Pair<T> {
    fn partial_cmp(&self, other: &Pair<T>) -> Option<Ordering> {
        match (self.x().partial_cmp(&other.x()), self.y().partial_cmp(&other.y())) {
            (Some(x), Some(y)) if x == y => Some(x),
            _ => None,
        }
    }
}

impl<T: Add + Copy> Add for Pair<T> {
    type Output = Pair<<T as Add>::Output>;

    fn add(self, other: Pair<T>) -> Self::Output {
        Pair([self.x() + other.x(), self.y() + other.y()])
    }
}

impl<T: Add + Copy> Pair<T> {
    pub fn sum(self) -> <T as Add>::Output {
        self.x() + self.y()
    }
}

impl<T: Copy + Sub> Sub for Pair<T> {
    type Output = Pair<<T as Sub>::Output>;

    fn sub(self, other: Pair<T>) -> Self::Output {
        Pair([self.x() - other.x(), self.y() - other.y()])
    }
}

impl<T: Copy + Mul> Mul for Pair<T> {
    type Output = Pair<<T as Mul>::Output>;

    fn mul(self, other: Pair<T>) -> Self::Output {
        Pair([self.x() * other.x(), self.y() * other.y()])
    }
}

impl<T: Copy + Div> Div for Pair<T> {
    type Output = Pair<<T as Div>::Output>;

    fn div(self, other: Pair<T>) -> Self::Output {
        Pair([self.x() / other.x(), self.y() / other.y()])
    }
}

pub type Point2D = Pair<f64>;

impl Point2D {
    pub fn zero() -> Self {
        Self([0.0, 0.0])
    }

    pub fn one() -> Self {
        Self([1.0, 1.0])
    }

    pub fn is_nan(&self) -> bool {
        self.x().is_nan() || self.y().is_nan()
    }
}

impl From<Point2D> for [f64; 2] {
    fn from(p: Point2D) -> [f64; 2] {
        p.0
    }
}

/// An `RTreeObject` that also carries data. Methods are simply forwarded to the `RTreeObject`.
#[derive(Clone)]
pub struct RTreeObjectWithData<S: RTreeObject, T>(pub S, pub T);

impl<S: RTreeObject, T> RTreeObject for RTreeObjectWithData<S, T> {
    type Envelope = <S as RTreeObject>::Envelope;

    fn envelope(&self) -> Self::Envelope {
        self.0.envelope()
    }
}

impl<S: RTreeObject + PointDistance, T> PointDistance for RTreeObjectWithData<S, T> {
    fn distance_2(
        &self,
        point: &<Self::Envelope as Envelope>::Point,
    ) -> <<Self::Envelope as Envelope>::Point as Point>::Scalar {
        self.0.distance_2(point)
    }
}

/// A quadrilateral. Used for interpolation between four points.
#[derive(Clone, Debug)]
pub struct Quad<V: Copy + Point> {
    pub points: [V; 4],
    pub edges: [Line<V>; 4],
}

impl<V: Copy + Point> Quad<V> {
    pub fn new(points: [V; 4]) -> Quad<V> {
        Quad {
            points,
            edges: [
                Line::new(points[0], points[1]),
                Line::new(points[1], points[2]),
                Line::new(points[2], points[3]),
                Line::new(points[3], points[0]),
            ],
        }
    }
}

impl RTreeObject for Quad<Point2D> {
    type Envelope = AABB<Point2D>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_points(self.points.iter())
    }
}

impl PointDistance for Quad<Point2D> {
    fn distance_2(&self, point: &Point2D) -> f64 {
        /// The winding number for a polygon with respect to a point: counts the number of times
        /// the polygon winds around the point. If the winding number is zero, then the point lies
        /// outside the polygon.
        /// This algorithm is based on the one at: http://geomalgorithms.com/a03-_inclusion.html.
        fn winding_number(point: &Point2D, points: &[Point2D; 4]) -> i8 {
            // The displacement of a point from a line
            // (in effect the determinant of a 2x2 matrix).
            fn displ(line: [Point2D; 2], point: Point2D) -> f64 {
                let [base, end] = line;
                let end = end - base;
                let point = point - base;
                end.x() * point.y() - end.y() * point.x()
            }

            (0..4).map(|i| {
                if (points[i].y() <= point.y()) != (points[(i + 1) % 4].y() <= point.y()) {
                    match displ([points[i], points[(i + 1) % 4]], *point) {
                        d if d > 0.0 => 1,
                        d if d < 0.0 => -1,
                        _ => 0,
                    }
                } else {
                    0
                }
            }).sum()
        }

        // The minimum distance from any edge to the point.
        let min_dis = self.edges.iter()
            .filter_map(|edge| OrdFloat::new(edge.distance_2(point)))
            .min()
            .unwrap()
            .into();

        if winding_number(&point, &self.points) == 0 {
            min_dis
        } else {
            // If the point is contained inside the shape, we must return a negative distance.
            -min_dis
        }
    }
}
