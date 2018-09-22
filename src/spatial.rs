use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Sub};

use spade::{BoundingRect, PointN, SpadeNum, SpatialObject, TwoDimensional};
use spade::primitives::SimpleEdge;

use approximation::OrdFloat;

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

impl<T: Copy + SpadeNum> PointN for Pair<T> {
    type Scalar = T;

    fn dimensions() -> usize {
        2
    }

    fn from_value(value: Self::Scalar) -> Self {
        Pair::diag(value)
    }

    fn nth(&self, index: usize) -> &Self::Scalar {
        &self.0[index]
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self.0[index]
    }
}

impl<T: Copy + SpadeNum> TwoDimensional for Pair<T> {}

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
