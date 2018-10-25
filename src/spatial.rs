use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Sub};

use serde::ser::{Serialize, Serializer};
use spade::{BoundingRect, PointN, SpatialObject};
use spade::primitives::SimpleEdge;
use spade::PointNExtensions;

use approximation::OrdFloat;

/// A cartesian point with some helper methods.
#[derive(Clone, Copy)]
pub struct Point2D([f64; 2]);

impl Point2D {
    pub fn new(p: [f64; 2]) -> Self {
        Self(p)
    }

    pub fn into_inner(self) -> [f64; 2] {
        self.into()
    }

    pub fn mul(self, multiplier: f64) -> Self {
        Self([self.0[0] * multiplier, self.0[1] * multiplier])
    }

    pub fn div(self, divisor: f64) -> Self {
        Self([self.0[0] / divisor, self.0[1] / divisor])
    }

    pub fn is_nan(&self) -> bool {
        self.0[0].is_nan() || self.0[1].is_nan()
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

impl PartialEq for Point2D {
    fn eq(&self, other: &Point2D) -> bool {
        self.0[0] == other.0[0] && self.0[1] == other.0[1]
    }
}

impl PartialOrd for Point2D {
    fn partial_cmp(&self, other: &Point2D) -> Option<Ordering> {
        match (self.0[0].partial_cmp(&other.0[0]), self.0[1].partial_cmp(&other.0[1])) {
            (Some(x), Some(y)) if x == y => Some(x),
            _ => None,
        }
    }
}

impl Add for Point2D {
    type Output = Point2D;

    fn add(self, other: Point2D) -> Point2D {
        Point2D([self.0[0] + other.0[0], self.0[1] + other.0[1]])
    }
}

impl Sub for Point2D {
    type Output = Point2D;

    fn sub(self, other: Point2D) -> Point2D {
        Point2D([self.0[0] - other.0[0], self.0[1] - other.0[1]])
    }
}

impl Mul for Point2D {
    type Output = Point2D;

    fn mul(self, other: Point2D) -> Point2D {
        Point2D([self.0[0] * other.0[0], self.0[1] * other.0[1]])
    }
}

impl Div for Point2D {
    type Output = Point2D;

    fn div(self, other: Point2D) -> Point2D {
        Point2D([self.0[0] / other.0[0], self.0[1] / other.0[1]])
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
    pub diam: V::Scalar,
}

impl<V: PointN + Copy> Quad<V> {
    pub fn new(points: [V; 4], zero: V::Scalar) -> Quad<V> {
        Quad {
            points,
            edges: [
                SimpleEdge::new(points[0], points[1]),
                SimpleEdge::new(points[1], points[2]),
                SimpleEdge::new(points[2], points[3]),
                SimpleEdge::new(points[3], points[0]),
            ],
            diam: zero,
        }
    }
}

impl SpatialObject for Quad<[f64; 2]> {
    type Point = [f64; 2];

    fn mbr(&self) -> BoundingRect<[f64; 2]> {
        BoundingRect::from_points(self.points.iter().cloned())
    }

    fn distance2(&self, point: &[f64; 2]) -> f64 {
        /// The winding number for a polygon with respect to a point: counts the number of times
        /// the polygon winds around the point. If the winding number is zero, then the point lies
        /// outside the polygon.
        /// This algorithm is based on the one at: http://geomalgorithms.com/a03-_inclusion.html.
        fn winding_number(point: &[f64; 2], points: &[[f64; 2]; 4]) -> i8 {
            // The displacement of a point from a line (in effect the determinant of a 2x2 matrix).
            fn displ(line: [[f64; 2]; 2], point: [f64; 2]) -> f64 {
                let [base, end] = line;
                let end = end.sub(&base);
                let point = point.sub(&base);
                end[0] * point[1] - end[1] * point[0]
            }

            (0..4).map(|i| {
                if (points[i][1] <= point[1]) != (points[(i + 1) % 4][1] <= point[1]) {
                    match displ([points[i], points[(i + 1) % 4]], *point) {
                        x if x > 0.0 => 1,
                        x if x < 0.0 => -1,
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

        if winding_number(point, &self.points) == 0 {
            min_dis
        } else {
            // If the point is contained inside the shape, we must return a negative distance.
            -min_dis
        }
    }
}
