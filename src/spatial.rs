use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Sub};

use serde::ser::{Serialize, Serializer};
use spade::{BoundingRect, PointN, SpatialObject, TwoDimensional};
use spade::primitives::SimpleEdge;

use approximation::OrdFloat;

/// A cartesian point with some helper methods.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point2D([f64; 2]);

impl Point2D {
    pub fn new(p: [f64; 2]) -> Self {
        Self(p)
    }

    pub fn zero() -> Self {
        Self([0.0, 0.0])
    }

    pub fn one() -> Self {
        Self([1.0, 1.0])
    }

    pub fn into_inner(self) -> [f64; 2] {
        self.into()
    }

    #[inline]
    pub fn x(&self) -> f64 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f64 {
        self.0[1]
    }

    pub fn mul(self, multiplier: f64) -> Self {
        Self([self.x() * multiplier, self.y() * multiplier])
    }

    pub fn div(self, divisor: f64) -> Self {
        Self([self.x() / divisor, self.y() / divisor])
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

impl Serialize for Point2D {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.0.serialize(serializer)
    }
}

impl PointN for Point2D {
    type Scalar = f64;

    fn dimensions() -> usize {
        2
    }

    fn from_value(value: Self::Scalar) -> Self {
        Point2D::new([value; 2])
    }

    fn nth(&self, index: usize) -> &Self::Scalar {
        &self.0[index]
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self.0[index]
    }
}

impl TwoDimensional for Point2D {}

impl PartialOrd for Point2D {
    fn partial_cmp(&self, other: &Point2D) -> Option<Ordering> {
        match (self.x().partial_cmp(&other.x()), self.y().partial_cmp(&other.y())) {
            (Some(x), Some(y)) if x == y => Some(x),
            _ => None,
        }
    }
}

impl Add for Point2D {
    type Output = Point2D;

    fn add(self, other: Point2D) -> Point2D {
        Point2D([self.x() + other.x(), self.y() + other.y()])
    }
}

impl Sub for Point2D {
    type Output = Point2D;

    fn sub(self, other: Point2D) -> Point2D {
        Point2D([self.x() - other.x(), self.y() - other.y()])
    }
}

impl Mul for Point2D {
    type Output = Point2D;

    fn mul(self, other: Point2D) -> Point2D {
        Point2D([self.x() * other.x(), self.y() * other.y()])
    }
}

impl Div for Point2D {
    type Output = Point2D;

    fn div(self, other: Point2D) -> Point2D {
        Point2D([self.x() / other.x(), self.y() / other.y()])
    }
}

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

/// A quadrilateral. Used for interpolation between four points.
#[derive(Clone, Debug)]
pub struct Quad<V: PointN + Copy> {
    pub points: [V; 4],
    pub edges: [SimpleEdge<V>; 4],
}

impl<V: PointN + Copy> Quad<V> {
    pub fn new(points: [V; 4]) -> Quad<V> {
        Quad {
            points,
            edges: [
                SimpleEdge::new(points[0], points[1]),
                SimpleEdge::new(points[1], points[2]),
                SimpleEdge::new(points[2], points[3]),
                SimpleEdge::new(points[3], points[0]),
            ],
        }
    }
}

impl SpatialObject for Quad<Point2D> {
    type Point = Point2D;

    fn mbr(&self) -> BoundingRect<Point2D> {
        BoundingRect::from_points(self.points.iter().cloned())
    }

    fn distance2(&self, point: &Point2D) -> f64 {
        /// The winding number for a polygon with respect to a point: counts the number of times
        /// the polygon winds around the point. If the winding number is zero, then the point lies
        /// outside the polygon.
        /// This algorithm is based on the one at: http://geomalgorithms.com/a03-_inclusion.html.
        fn winding_number(point: &Point2D, points: &[Point2D; 4]) -> i8 {
            // The displacement of a point from a line (in effect the determinant of a 2x2 matrix).
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
            .filter_map(|edge| OrdFloat::new(edge.distance2(point)))
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
