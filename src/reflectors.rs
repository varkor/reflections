use std::collections::HashSet;

use spade::PointN;
use spade::PointNExtensions;
use spade::primitives::SimpleEdge;
use spade::rtree::RTree;
use spade::SpatialObject;

use approximation::{Interval, View};
// use approximation::adaptive_sample;
use approximation::Equation;
// use approximation::KeyValue;
use approximation::OrdFloat;
use spatial::{Point2D, Quad, SpatialObjectWithData};

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
        glide: f64,
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
                    let reflection = match (scale == 1.0, translate == 0.0) {
                        (true, true) => point,
                        (_, true) => (normal.function)(s * scale),
                        (_, false) => (mirror.normal(t + translate).function)(s * scale),
                    };
                    grid[x as usize + y as usize * cols].push(reflection);
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
        _interval: &Interval,
        _: &View,
        scale: f64,
        glide: f64,
    ) -> Vec<Point2D> {
        let mut pairs = vec![];

        let samps1: Vec<_> =
        (Interval { start: -256.0, end: 256.0, step: 1.0 })
        // adsamp.into_iter()

        .map(|t| {
            println!("{}", t);
            let normal = mirror.normal(t);
            let samps: Vec<(Point2D, Point2D, (f64, f64))> = (Interval { start: -256.0, end: 256.0, step: 512.0 }).filter_map(|s| {
                let nfs = (normal.function)(s);
                let nfms = match (scale == 1.0, glide == 0.0) {
                    (true, true) => nfs,
                    (_, true) => (normal.function)(s * scale),
                    (_, false) => (mirror.normal(t + glide).function)(s * scale),
                };
                if !nfs.is_nan() && !nfms.is_nan() {
                    Some((nfs, nfms, (t, s)))
                } else {
                    None
                }
            }).collect();
            samps
        }).collect();
        let windows1 = samps1.windows(2);

        for window1 in windows1.into_iter() {
            if let &[ref wins1, ref wins2] = window1 {
                let wins1: Vec<_> = wins1.windows(2).collect();
                let wins2: Vec<_> = wins2.windows(2).collect();
                for i in 0..wins1.len() {
                    let (l, r) = (wins1[i], wins2[i]);
                    if let (&[s11, s12], &[s21, s22]) = (l, r) {
                        let mut quad = Quad::new([
                            s11.0.into_inner(), // FIXME
                            s12.0.into_inner(),
                            s22.0.into_inner(),
                            s21.0.into_inner(),
                        ], 0.0);
                        quad.diam = [1, 2, 3].iter().map(|&i: &usize| OrdFloat(quad.points[0].sub(&quad.points[i]).length2())).max().unwrap().0.sqrt();
                        pairs.push(SpatialObjectWithData(
                            quad,
                            (s11.1, s12.1, s22.1, s21.1),
                        ));
                    }
                }
            }
        }

        let rtree = RTree::bulk_load(pairs);

        let mut reflection = HashSet::new();

        // let figure_sample = adaptive_sample(
        //     |t| {
        //         // log(&format!("{:?}", t));
        //         let (x, y) = (figure.function)(t);
        //         KeyValue(Point2D(x, y), (x, y))
        //     },
        //     &range,
        //     samples * 2,
        // );
        let interval_sample = figure.sample(&(Interval { start: -256.0, end: 256.0, step: 0.5 }));

        // let threshold = thresh.sqrt();

        fn projection_on_edge<V: PointN>(edge: &SimpleEdge<V>, query_point: &V) -> V::Scalar {
            let (p1, p2) = (&edge.from, &edge.to);
            let dir = p2.sub(p1);
            let s = query_point.sub(p1).dot(&dir);
            s
        }

        // let fs = figure_sample;
        let fs = interval_sample;

        for p in fs {
            if p.is_nan() {
                continue;
            }

            let p = p.into_inner();
            for SpatialObjectWithData(quad, (v1, v2, v3, v4)) in rtree.lookup_in_circle(&p, &0.0) {
                let a = projection_on_edge(&quad.edges[0], &p) / quad.edges[0].length2();
                let a_dis = quad.edges[0].distance2(&p);
                let b = 1.0 - projection_on_edge(&quad.edges[2], &p) / quad.edges[2].length2();
                let b_dis = quad.edges[2].distance2(&p);
                let total_dis = a_dis + b_dis;
                let a_factor = 1.0 - a_dis / total_dis;
                let b_factor = 1.0 - b_dis / total_dis;

                let ad = *v2 - *v1; // FIXME
                let a = *v1 + ad.mul(a);
                let bd = *v3 - *v4;
                let b = *v4 + bd.mul(b);
                let p = a.mul(a_factor) + b.mul(b_factor);
                let [x, y] = p.into_inner();

                // let (adx, ady) = (v2.0 - v1.0, v2.1 - v1.1);
                // let (ax, ay) = (v1.0 + adx * a, v1.1 + ady * a);
                // let (bdx, bdy) = (v3.0 - v4.0, v3.1 - v4.1);
                // let (bx, by) = (v4.0 + bdx * b, v4.1 + bdy * b);
                // let (x, y) = (a_factor * ax + b_factor * bx, a_factor * ay + b_factor * by);
                reflection.insert((x.to_bits(), y.to_bits()));
            }
        }

        // FIXME
        reflection.iter().map(|(x, y)| Point2D::new([f64::from_bits(*x), f64::from_bits(*y)])).collect()
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
