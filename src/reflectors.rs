use std::collections::{HashMap, HashSet};

use spade::{PointN, PointNExtensions /* FIXME */, SpatialObject};
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
                if let Some([x, y]) = view.project(point) {
                    // In some cases, we can use cached computations to calculate the reflections.
                    let image = match (scale == 1.0, translate == 0.0) {
                        (true, true) => point,
                        (_, true) => (normal.function)(s * scale),
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
            if let Some(cell) = view.project(point) {
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
                        (_, true) => (normal.function)(s * scale),
                        (_, false) => (mirror.normal(t + translate).function)(s * scale),
                    };
                    if !image.is_nan() {
                        return Some(ReflectedPair { point, image });
                    }
                }

                None
            }).collect::<Vec<_>>()
        }).collect();

        // A collection of quads with (t, s) data at each point, used for image interpolation.
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

        for point in figure.sample(&interval).into_iter().filter(|point| !point.is_nan()) {
            rtree.lookup_in_circle(&point, &0.0).iter().for_each(|quad| {
                reflection.entry((quad.1).0).or_insert(vec![]).push(point);
            });
        }

        fn projection_on_edge<V: PointN>(edge: &SimpleEdge<V>, query_point: &V) -> V::Scalar {
            let (p1, p2) = (&edge.from, &edge.to);
            let dir = p2.sub(p1);
            let s = query_point.sub(p1).dot(&dir);
            s
        }

        reflection.into_iter()
            .map(|(index, points)| (reflection_regions[index].clone(), points))
            .flat_map(|(SpatialObjectWithData(quad, (_, (a, b, c, d))), points)| {
                points.iter().map(|point| {
                    // Interpolate the possible reflections corresponding to the quad vertices in
                    // comparison to the point.
                    let proj = Point2D::new([
                        projection_on_edge(&quad.edges[0], &point) / quad.edges[0].length2(),
                        1.0 - projection_on_edge(&quad.edges[2], &point) / quad.edges[2].length2(),
                    ]);
                    let dis = Point2D::new([
                        quad.edges[0].distance2(&point),
                        quad.edges[2].distance2(&point),
                    ]);
                    let factor = Point2D::one() - dis.div(dis.sum());
                    let [base, end] = [Pair::new([a, d]), Pair::new([b, c])];

                    ((base + (end - base) * Pair::diag(proj)) * Pair::diag(factor)).sum()
                }).collect::<Vec<_>>()
            })
            .collect()
    }
}

pub struct LinearApproximator(pub f64);

impl ReflectionApproximator for LinearApproximator {
    fn approximate_reflection(
        &self,
        mirror: &Equation,
        figure: &Equation,
        interval: &Interval,
        _view: &View,
        _scale: f64,
        _glide: f64,
    ) -> Vec<Point2D> {
        let mut pairs = vec![];

        let _range = interval.start..=interval.end;
        let _samples = ((interval.end - interval.start) / interval.step) as u64 + 1;

        for t in (Interval { start: -256.0, end: 256.0, step: 1.0 }) {
            let normal = mirror.normal(t);
            // should be able to reduce sampling significantly here (only when linear)
            let samps: Vec<(Point2D, Point2D)> = (Interval { start: -256.0, end: 256.0, step: 512.0 }).map(|s| {
                ((normal.function)(s), (normal.function)(-s))
            }).collect();
            let windows = samps.windows(2);
            for window in windows {
                if let &[(p1, v1), (p2, v2)] = window {
                    let p1 = p1.into_inner();
                    let p2 = p2.into_inner();
                    pairs.push(SpatialObjectWithData(SimpleEdge::new(p1, p2), (v1, v2)));
                }
            }
            // for s in interval.clone() {
            //     let (x, y) = (normal.function)(s);
            //     norm.push((x, y));
            //     pairs.push(SpatialObjectWithData([x, y], (normal.function)(-s)));
            // }
        }

        let rtree = RTree::bulk_load(pairs);

        let mut reflection = HashSet::new();

        /*let figure_sample = adaptive_sample(
            |t| {
                // log(&format!("{:?}", t));
                let (x, y) = (figure.function)(t);
                KeyValue(Point2D(x, y), (x, y))
            },
            &range,
            samples,
        );*/
        let interval_sample = figure.sample(&(Interval { start: -256.0, end: 256.0, step: 1.0 }));

        let threshold = self.0.sqrt();

        fn projection_on_edge<V: PointN>(edge: &SimpleEdge<V>, query_point: &V) -> V::Scalar {
            let (p1, p2) = (&edge.from, &edge.to);
            let dir = p2.sub(p1);
            let s = query_point.sub(p1).dot(&dir);
            s
        }

        // let fs = figure_sample;
        let fs = interval_sample;

        for p in fs {
            let p = p.into_inner(); // FIXME
            for SpatialObjectWithData(fig, (v1, v2)) in rtree.lookup_in_circle(&p, &threshold) {
                // find closest point (x, y) on line as param from 0 to 1
                let s = projection_on_edge(fig, &p) / fig.length2(); // need to check for DBZ
                if s >= 0.0 && s <= 1.0 {
                    // maybe we should check 0 <= s <= 1?
                    let d = *v2 - *v1;
                    // calc 0-1 param on refl
                    let p = *v1 + d.mul(s);
                    let [x, y] = p.into_inner();
                    reflection.insert((x.to_bits(), y.to_bits()));
                }
            }
        }

        reflection.iter().map(|(x, y)| Point2D::new([f64::from_bits(*x), f64::from_bits(*y)])).collect()
    }
}
