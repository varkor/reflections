use std::collections::HashSet;

use spade::BoundingRect;
use spade::PointN;
use spade::PointNExtensions;
use spade::primitives::SimpleEdge;
use spade::rtree::RTree;
use spade::SpatialObject;

use approximation::{Interval, View};
use approximation::adaptive_sample;
use approximation::Equation;
use approximation::KeyValue;
use approximation::OrdFloat;
use approximation::SpatialObjectWithPayload;

/// A `ReflectionApproximator` provides a method to approximate points lying along the reflection
/// of a `figure` equation in a `mirror` equation.
pub trait ReflectionApproximator {
    fn approximate_reflection(
        &self,
        mirror: &Equation,
        figure: &Equation,
        interval: &Interval,
        view: &View,
    ) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>);
}

/// Approximation of a reflection using a rasterisation technique: splitting the view up into a grid
/// and sampling cells to find those containing points in the reflection. This tends to be accurate,
/// but can be slow for finer grids.
pub struct RasterisationApproximator;

impl ReflectionApproximator for RasterisationApproximator {
    fn approximate_reflection(
        &self,
        mirror: &Equation,
        figure: &Equation,
        interval: &Interval,
        view: &View,
    ) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
        let mut grid = vec![vec![]; (view.cols as usize) * (view.rows as usize)];
        let mut normals = vec![];

        // Generate the normal mappings.
        for t in interval.iter() {
            let normal = mirror.normal(t);
            let mut norm = vec![];
            for s in interval.iter() {
                let z = (normal.function)(s);
                norm.push(z);
                if let Some((x, y)) = view.project(z) {
                    grid[x + y * (view.cols as usize)].push((normal.function)(-s));
                }
            }
            normals.push(norm);
        }

        // Intersect the grid with the figure.
        let mut reflection = HashSet::new();
        for point in figure.sample(&interval) {
            if let Some((x, y)) = view.project(point) {
                for point in &grid[x + y * (view.cols as usize)] {
                    let (x, y) = point;
                    reflection.insert((x.to_bits(), y.to_bits()));
                }
            }
        }

        (reflection.into_iter().map(|(x, y)| {
            (f64::from_bits(x), f64::from_bits(y))
        }).collect(), normals)
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
    ) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
        let mut pairs = vec![];
        let norms = vec![];

        // let range = interval.start..=interval.end;
        // let samples = ((interval.end - interval.start) / interval.step) as u64 + 1;

        #[derive(Clone, Debug)]
        struct Quad<V: PointN + Copy> {
            points: [V; 4],
            edges: [SimpleEdge<V>; 4],
            diam: V::Scalar,
        }

        impl<V: PointN + Copy> Quad<V> {
            fn new(points: [V; 4], zero: V::Scalar) -> Quad<V> {
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

                let is_left = |p0: [f64; 2], p1: [f64; 2], p2: [f64; 2]| {
                    (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] -  p0[0]) * (p1[1] - p0[1])
                };

                // http://geomalgorithms.com/a03-_inclusion.html
                let winding_number = || {
                    let mut wn = 0;

                    for i in 0..4 {
                        if self.points[i][1] <= point[1] {
                            if self.points[(i + 1) % 4][1] > point[1] {
                                if is_left(self.points[i], self.points[(i + 1) % 4], *point) > 0.0 {
                                    wn += 1;
                                }
                            }
                        } else {
                            if self.points[(i + 1) % 4][1] <= point[1] {
                                if is_left(self.points[i], self.points[(i + 1) % 4], *point) < 0.0 {
                                    wn -= 1;
                                }
                            }
                        }
                    }

                    wn
                };

                let min_dis = self.edges.iter().map(|edge| OrdFloat(edge.distance2(point))).min().unwrap().0;

                if winding_number() == 0 {
                    min_dis
                } else {
                    -min_dis
                }
            }
        }

        // let mut adsamp = adaptive_sample(|t| KeyValue((), t), &(-256.0..=256.0), 513);
        let mut adsamp = adaptive_sample(|t| {
            // let (x, y) = (mirror.derivative().function)(t);
            // println!("(x,y)={:?} t={} l={}", (x, y), t, [x, y].length2());
            // KeyValue([x, y].length2(), t)
            // KeyValue(mirror.gradient(t), t)
            KeyValue((), t)
        }, &(-256.0..=256.0), 513);
        adsamp.sort_unstable_by_key(|&x| OrdFloat(x));

        let samps1: Vec<_> =
        // (Interval { start: -256.0, end: 256.0, step: 1.0 }).iter()
        adsamp.into_iter()

        .map(|t| {
            println!("{}", t);
            let normal = mirror.normal(t);
            let samps: Vec<((f64, f64), (f64, f64), (f64, f64))> = (Interval { start: -256.0, end: 256.0, step: 512.0 }).iter().filter_map(|s| {
                let nfs = (normal.function)(s);
                let nfms = (normal.function)(-s);
                if !nfs.0.is_nan() && !nfs.1.is_nan() && !nfms.0.is_nan() && !nfms.1.is_nan() {
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
                            [(s11.0).0, (s11.0).1],
                            [(s12.0).0, (s12.0).1],
                            [(s22.0).0, (s22.0).1],
                            [(s21.0).0, (s21.0).1],
                        ], 0.0);
                        quad.diam = [1, 2, 3].iter().map(|&i: &usize| OrdFloat(quad.points[0].sub(&quad.points[i]).length2())).max().unwrap().0.sqrt();
                        pairs.push(SpatialObjectWithPayload(
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

        for (x, y) in fs {
            if x.is_nan() || y.is_nan() {
                continue;
            }

            let p = &[x, y];
            for SpatialObjectWithPayload(quad, (v1, v2, v3, v4)) in rtree.lookup_in_circle(p, &0.0) {


                let a = projection_on_edge(&quad.edges[0], p) / quad.edges[0].length2();
                let a_dis = quad.edges[0].distance2(p);
                let b = 1.0 - projection_on_edge(&quad.edges[2], p) / quad.edges[2].length2();
                let b_dis = quad.edges[2].distance2(p);
                let total_dis = a_dis + b_dis;
                let a_factor = 1.0 - a_dis / total_dis;
                let b_factor = 1.0 - b_dis / total_dis;
                let (adx, ady) = (v2.0 - v1.0, v2.1 - v1.1);
                let (ax, ay) = (v1.0 + adx * a, v1.1 + ady * a);
                let (bdx, bdy) = (v3.0 - v4.0, v3.1 - v4.1);
                let (bx, by) = (v4.0 + bdx * b, v4.1 + bdy * b);
                let (x, y) = (a_factor * ax + b_factor * bx, a_factor * ay + b_factor * by);
                reflection.insert((x.to_bits(), y.to_bits()));
            }
        }

        (reflection.iter().map(|(x, y)| (f64::from_bits(*x), f64::from_bits(*y))).collect(), norms)
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
    ) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
        let mut pairs = vec![];
        let mut norms = vec![];

        let _range = interval.start..=interval.end;
        let _samples = ((interval.end - interval.start) / interval.step) as u64 + 1;

        for t in (Interval { start: -256.0, end: 256.0, step: 1.0 }).iter() {
            let normal = mirror.normal(t);
            let mut norm = vec![];
            // should be able to reduce sampling significantly here (only when linear)
            let samps: Vec<((f64, f64), (f64, f64))> = (Interval { start: -256.0, end: 256.0, step: 512.0 }).iter().map(|s| {
                ((normal.function)(s), (normal.function)(-s))
            }).collect();
            for ((x, y), _) in &samps {
                norm.push((*x, *y));
            }
            let windows = samps.windows(2);
            for window in windows {
                if let &[((x1, y1), v1), ((x2, y2), v2)] = window {
                    pairs.push(SpatialObjectWithPayload(SimpleEdge::new([x1, y1], [x2, y2]), (v1, v2)));
                }
            }
            // for s in interval.iter() {
            //     let (x, y) = (normal.function)(s);
            //     norm.push((x, y));
            //     pairs.push(SpatialObjectWithPayload([x, y], (normal.function)(-s)));
            // }
            norms.push(norm);
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

        for (x, y) in fs {
            let p = &[x, y];
            for SpatialObjectWithPayload(fig, (v1, v2)) in rtree.lookup_in_circle(p, &threshold) {
                // find closest point (x, y) on line as param from 0 to 1
                let s = projection_on_edge(fig, p) / fig.length2(); // need to check for DBZ
                if s >= 0.0 && s <= 1.0 {
                    // maybe we should check 0 <= s <= 1?
                    let (dx, dy) = (v2.0 - v1.0, v2.1 - v1.1);
                    // calc 0-1 param on refl
                    let (x, y) = (v1.0 + dx * s, v1.1 + dy * s);
                    reflection.insert((x.to_bits(), y.to_bits()));
                }
            }
        }

        (reflection.iter().map(|(x, y)| (f64::from_bits(*x), f64::from_bits(*y))).collect(), norms)
    }
}
