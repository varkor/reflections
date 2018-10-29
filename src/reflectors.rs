use std::collections::{HashMap, HashSet};

use spade::SpatialObject;
use spade::primitives::SimpleEdge;
use spade::rtree::RTree;

use approximation::{Equation, Interval, View};
use spatial::{Pair, Point2D, Quad, SpatialObjectWithData};

/// A `ReflectionApproximator` provides a method to approximate points lying along the reflection
/// of a `figure` equation in a `mirror` equation.
pub trait ReflectionApproximator {
    fn approximate_reflection(
        &self,
        mirror: &Equation,
        figure: &Equation,
        interval: &Interval,
        view: &View,
        scale: f64,
        translate: f64,
    ) -> Vec<Point2D>;
}

/// Find the distance of a point projected along an edge.
fn projection_on_edge(edge: &SimpleEdge<Point2D>, p: Point2D) -> f64 {
    ((p - edge.from) * (edge.to - edge.from)).sum()
}

/// Approximation of a reflection using a rasterisation technique: splitting the view up into a grid
/// and sampling cells to find those containing points in the reflection. This tends to be accurate,
/// but can be slow for finer grids.
pub struct RasterisationApproximator {
    /// The size of each rasterisation cell in pixels.
    pub cell_size: u16,
}

impl ReflectionApproximator for RasterisationApproximator {
    fn approximate_reflection(
        &self,
        mirror: &Equation,
        figure: &Equation,
        interval: &Interval,
        view: &View,
        scale: f64,
        translate: f64,
    ) -> Vec<Point2D> {
        // Calculate the number of cells we need horizontally and vertically. Round up if the view
        // size isn't perfectly divisible by the cell size.
        let [cols, rows] = [
            ((view.width + self.cell_size - 1) / self.cell_size) as usize,
            ((view.height + self.cell_size - 1) / self.cell_size) as usize,
        ];
        // Each cell (corresponding to a region) contains mappings from points in that region
        // to their reflections.
        let mut grid = vec![vec![]; cols * rows];

        // Populate the mapping grid.
        for t in interval.clone() {
            let normal = mirror.normal(t);
            for s in interval.clone() {
                let point = (normal.function)(s);
                if let Some([x, y]) = view.project(point, [cols, rows]) {
                    // In some cases, we can use cached computations to calculate the reflections.
                    let image = match (scale == 1.0, translate == 0.0) {
                        (true, true) => point,
                        (false, true) => (normal.function)(s * scale),
                        (_, false) => (mirror.normal(t + translate).function)(s * scale),
                    };
                    grid[x as usize + y as usize * cols].push(image);
                }
            }
        }

        // Intersect the grid with the figure equation, determining all the points corresponding
        // to reflections of points on the figure.
        let mut reflection = HashSet::new();
        for point in figure.sample(&interval) {
            if let Some(cell) = view.project(point, [cols, rows]) {
                reflection.insert(cell);
            }
        }

        reflection.into_iter().flat_map(|[x, y]| {
            &grid[x as usize + y as usize * cols]
        }).cloned().collect()
    }
}

pub struct QuadraticApproximator;

impl ReflectionApproximator for QuadraticApproximator {
    fn approximate_reflection(
        &self,
        mirror: &Equation,
        figure: &Equation,
        interval: &Interval,
        _: &View,
        scale: f64,
        translate: f64,
    ) -> Vec<Point2D> {
        /// A pair corresponding to an image and its reflection.
        #[derive(Clone, Copy)]
        struct ReflectedPair {
            point: Point2D,
            image: Point2D,
        }

        // Sample points in (t, s) space.
        let samples: Vec<_> = interval.clone().map(|t| {
            let normal = mirror.normal(t);
            let endpoint_interval = Interval::endpoints(interval.start, interval.end);

            endpoint_interval.filter_map(|s| {
                let point = (normal.function)(s);

                if !point.is_nan() {
                    // In some cases, we can use cached computations to calculate the reflections.
                    let image = match (scale == 1.0, translate == 0.0) {
                        (true, true) => point,
                        (false, true) => (normal.function)(s * scale),
                        (_, false) => (mirror.normal(t + translate).function)(s * scale),
                    };
                    if !image.is_nan() {
                        return Some(ReflectedPair { point, image });
                    }
                }

                None
            }).collect::<Vec<_>>()
        }).collect();

        // A collection of quads with (point, image) data at each point, used for
        // image interpolation.
        let mut reflection_regions = vec![];

        // Populate `reflection_regions`.
        for t_pair in samples.windows(2).into_iter() {
            // This pattern match is guaranteed, but unfortuantely, `windows` doesn't contain
            // slice size information in its type.
            if let [sample_l, sample_r] = t_pair {
                for (l, r) in sample_l.windows(2).zip(sample_r.windows(2)) {
                    // The left and right sides are both similarly directed, but we want to create
                    // an anticlockwise quad, so we need to flip the order of the vertices on the
                    // right.
                    // Again, this pattern match is guaranteed.
                    if let (&[a, b], &[d, c]) = (l, r) {
                        let mut quad = Quad::new([a.point, b.point, c.point, d.point]);
                        let index = reflection_regions.len();
                        reflection_regions.push(SpatialObjectWithData(
                            quad, // FIXME: the data should be associated with each point.
                            (index, (a.image, b.image, c.image, d.image)),
                        ));
                    }
                }
            }
        }

        // Store the regions spatially, so we can lookup points within those regions.
        let rtree = RTree::bulk_load(reflection_regions.clone());

        let mut reflection = HashMap::new();

        // Sample points along the figure and find all quads within which they lie.
        for point in figure.sample(&interval).into_iter().filter(|point| !point.is_nan()) {
            rtree.lookup_in_circle(&point, &0.0).iter().for_each(|quad| {
                reflection.entry((quad.1).0).or_insert(vec![]).push(point);
            });
        }

        reflection.into_iter()
            .map(|(index, points)| (reflection_regions[index].clone(), points))
            .flat_map(|(SpatialObjectWithData(quad, (_, (a, b, c, d))), points)| {
                points.into_iter().map(|point| {
                    // Interpolate the possible reflections corresponding to the quad vertices in
                    // comparison to the point.
                    let proj = Pair::new([
                        projection_on_edge(&quad.edges[0], point) / quad.edges[0].length2(),
                        1.0 - projection_on_edge(&quad.edges[2], point) / quad.edges[2].length2(),
                    ]);
                    let dis = Point2D::new([
                        quad.edges[0].distance2(&point),
                        quad.edges[2].distance2(&point),
                    ]);
                    let factor = Point2D::one() - dis / Point2D::diag(dis.sum());
                    let [base, end] = [Pair::new([a, d]), Pair::new([b, c])];

                    ((base + (end - base) * proj.map(Pair::diag)) * factor.map(Pair::diag)).sum()
                }).collect::<Vec<_>>()
            })
            .collect()
    }
}

pub struct LinearApproximator {
    pub threshold: f64,
}

impl ReflectionApproximator for LinearApproximator {
    fn approximate_reflection(
        &self,
        mirror: &Equation,
        figure: &Equation,
        interval: &Interval,
        _view: &View,
        scale: f64,
        translate: f64,
    ) -> Vec<Point2D> {
        // A collection of lines with (point, image) data at each point, used for
        // image interpolation.
        let mut reflection_lines = vec![];

        // Sample points along the mirror, mapping points (t, s) to their images.
        for t in interval.clone() {
            let normal = mirror.normal(t);
            let endpoint_interval = Interval::endpoints(interval.start, interval.end);

            let samples: Vec<_> = endpoint_interval.map(|s| {
                let point = (normal.function)(s);
                let image = match (scale == 1.0, translate == 0.0) {
                    (true, true) => point,
                    (false, true) => (normal.function)(s * scale),
                    (_, false) => (mirror.normal(t + translate).function)(s * scale),
                };
                (point, image)
            }).collect();

            for window in samples.windows(2) {
                // Guaranteed to pattern match successfully.
                if let &[(point_l, image_l), (point_r, image_r)] = window {
                    let index = reflection_lines.len();
                    reflection_lines.push(SpatialObjectWithData(
                        SimpleEdge::new(point_l, point_r),
                        (index, (image_l, image_r)),
                    ));
                }
            }
        }

        let rtree = RTree::bulk_load(reflection_lines.clone());
        let mut reflection = HashMap::new();

        let threshold = self.threshold.sqrt();

        // Sample points along the figure, finding the closest line segment along the mirror and
        // interpolating the reflection image.
        for point in figure.sample(&interval) {
            rtree.lookup_in_circle(&point, &threshold).iter().for_each(|line| {
                reflection.entry((line.1).0).or_insert(vec![]).push(point);
            });
        }

        reflection.into_iter()
            .map(|(index, points)| (reflection_lines[index].clone(), points))
            .flat_map(|(SpatialObjectWithData(fig, (_, (base, end))), points)| {
                points.into_iter().filter_map(|point| {
                    // Find the closest point on the line `fig` to the point `p` as a parameter from
                    // 0 to 1.
                    let s = projection_on_edge(&fig, point);
                    if s >= 0.0 && s <= fig.length2() {
                        Some(base + (end - base) * Point2D::diag(s / fig.length2()))
                    } else {
                        None
                    }
                }).collect::<Vec<_>>()
            })
            .collect()
    }
}
