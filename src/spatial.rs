use spade::{BoundingRect, PointN, SpatialObject};

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
