use spade::{BoundingRect, PointN, SpatialObject};
use spade::primitives::SimpleEdge;
use spade::PointNExtensions;

use approximation::OrdFloat;

/// FIXME: use an array instead.
pub type Point2D = (f64, f64);

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
