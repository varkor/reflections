use spade::{BoundingRect, PointN, SpatialObject};
use spade::primitives::SimpleEdge;

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
            // The displacement of the point `p2` from the line `pl`.
            // FIXME
            fn displ(pl: [[f64; 2]; 2], p2: [f64; 2]) -> f64 {
                let [p0, p1] = pl;
                (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] -  p0[0]) * (p1[1] - p0[1])
            }

            (0..4).map(|i| {
                match (points[i][1] <= point[1], points[(i + 1) % 4][1] <= point[1]) {
                    (true, false) if displ([points[i], points[(i + 1) % 4]], *point) > 0.0 => 1,
                    (false, true) if displ([points[i], points[(i + 1) % 4]], *point) < 0.0 => -1,
                    _ => 0,
                }
            }).sum()
        }

        let min_dis = self.edges.iter().filter_map(|edge| OrdFloat::new(edge.distance2(point))).min().unwrap().0;

        if winding_number(point, &self.points) == 0 {
            min_dis
        } else {
            -min_dis
        }
    }
}
