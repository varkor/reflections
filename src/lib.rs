#![feature(box_syntax)]
#![feature(try_trait)]
#![feature(self_struct_ctor)]
#![feature(euclidean_division)]

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
extern crate serde_json;
extern crate wasm_bindgen;
extern crate kdtree;
extern crate spade;

use wasm_bindgen::prelude::*;
use kdtree::KdTree;
use kdtree::distance::squared_euclidean;
use spade::rtree::RTree;
use spade::SpatialObject;
use spade::PointN;
use spade::BoundingRect;
use spade::primitives::SimpleEdge;
use spade::PointNExtensions;

use std::collections::{HashSet, HashMap, BinaryHeap};
use std::cmp::Ordering;
use std::f64::consts::PI;
use std::ops::RangeInclusive;
use std::ops::Sub;
use std::rc::Rc;
use std::fmt::Debug;

pub mod parser;
use parser::{Lexer, Parser};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

struct Interval {
    pub start: f64,
    pub end: f64,
    pub step: f64,
}

struct IntervalIter {
    cur: f64,
    end: f64,
    step: f64,
}

impl Interval {
    fn iter(&self) -> IntervalIter {
        IntervalIter {
            cur: self.start,
            end: self.end,
            step: self.step,
        }
    }
}

impl Iterator for IntervalIter {
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur > self.end {
            None
        } else {
            let cur = self.cur;
            self.cur += self.step;
            Some(cur)
        }
    }
}

trait Delta {
    type Output: Ord;

    fn delta(&self, &Self) -> Self::Output;
}

#[derive(Clone, Copy)]
struct Angle(f64);

impl Angle {
    fn new(a: f64) -> Self {
        Self(a.mod_euc(2.0 * PI))
    }
}

impl Delta for Angle {
    type Output = OrdFloat;

    fn delta(&self, other: &Self) -> Self::Output {
        OrdFloat((self.0 - other.0 + PI).mod_euc(2.0 * PI) - PI)
    }
}

struct Equation {
    function: Box<Fn(f64) -> (f64, f64)>,
}

fn derivative(f: &Box<Fn(f64) -> (f64, f64)>, t: f64) -> (f64, f64) {
    let h = 0.1;
    let (fp, fm) = (f(t + h), f(t - h));
    let d = 2.0 * h;
    ((fp.0 - fm.0) / d, (fp.1 - fm.1) / d)
}

impl Equation {
    fn sample(&self, interval: &Interval) -> Vec<(f64, f64)> {
        interval.iter().map(|t| (self.function)(t)).collect()
    }

    fn normal(&self, t: f64) -> Equation {
        let (mx, my) = (self.function)(t);
        let (dx, dy) = derivative(&self.function, t);

        Equation { function: Box::new(move |s| {
            (mx - s * dy, my + s * dx)
        }) }
    }

    fn gradient(&self, t: f64) -> Angle {
        let (dx, dy) = derivative(&self.function, t);
        Angle::new(dy.atan2(dx))
    }
}

struct View {
    cols: u16,
    rows: u16,
    size: f64,
    x: f64,
    y: f64,
}

impl View {
    fn project(&self, (x, y): (f64, f64)) -> Option<(usize, usize)> {
        let (x, y) = (((x - self.x) / self.size).round(), ((y - self.y) / self.size).round());
        if x >= 0.0 && y >= 0.0 {
            let (x, y) = (x as usize, y as usize);
            if x < self.cols as usize && y < self.rows as usize {
                return Some((x, y));
            }
        }
        None
    }
}

fn approximate_reflection_vis(
    mirror: &Equation,
    figure: &Equation,
    interval: &Interval,
    view: &View,
    thresh: f64,
) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
    let mut grid = vec![vec![]; (view.cols as usize) * (view.rows as usize)];
    let mut norms = vec![];
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
        norms.push(norm);
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
    (reflection.iter().map(|(x, y)| (f64::from_bits(*x), f64::from_bits(*y))).collect(), norms)
}

fn approximate_reflection_kd(
    mirror: &Equation,
    figure: &Equation,
    interval: &Interval,
    _view: &View,
    thresh: f64,
) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
    let mut pairs = vec![];
    let mut norms = vec![];

    let mut kdtree = KdTree::new(2);

    for t in interval.iter() {
        let normal = mirror.normal(t);
        let mut norm = vec![];
        for s in interval.iter() {
            let (x, y) = (normal.function)(s);
            norm.push((x, y));
            pairs.push(([x, y], (normal.function)(-s)));
        }
        norms.push(norm);
    }

    for (p, (s, t)) in &pairs {
        kdtree.add(p, (s, t)).unwrap();
    }

    let mut reflection = HashSet::new();

    for (x, y) in figure.sample(&interval) {
        if let Ok(iter) = kdtree.iter_nearest(&[x, y], &squared_euclidean) {
            for (dist, point) in iter {
                if dist > thresh {
                    break;
                }
                let (x, y) = point;
                reflection.insert((x.to_bits(), y.to_bits()));
            }
        }
    }

    (reflection.iter().map(|(x, y)| (f64::from_bits(*x), f64::from_bits(*y))).collect(), norms)
}

#[derive(Clone, Copy)]
struct KeyValue<K, V>(K, V);

impl<K: PartialEq, V> PartialEq for KeyValue<K, V> {
    fn eq(&self, other: &KeyValue<K, V>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K: Eq, V> Eq for KeyValue<K, V> {}

impl<K: PartialOrd, V> PartialOrd for KeyValue<K, V> {
    fn partial_cmp(&self, other: &KeyValue<K, V>) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<K: Ord, V> Ord for KeyValue<K, V> {
    fn cmp(&self, other: &KeyValue<K, V>) -> Ordering {
        self.0.cmp(&other.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct OrdFloat(f64);

impl PartialEq for OrdFloat {
    fn eq(&self, other: &OrdFloat) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for OrdFloat {}

impl PartialOrd for OrdFloat {
    fn partial_cmp(&self, other: &OrdFloat) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrdFloat {
    fn cmp(&self, other: &OrdFloat) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

// FIXME: replace this with an iterator
fn adaptive_sample<K: Clone + Delta, V: Clone, F: Fn(f64) -> KeyValue<K, V>>(
    f: F,
    range: &RangeInclusive<f64>,
    samples: u64,
) -> Vec<V>
    where <K as Delta>::Output: Ord + Debug,
{
    // println!("adaptive_sample");
    assert!(samples >= 2);

    let mut pq = BinaryHeap::new();

    let evaled_pair = |t: f64| -> (f64, KeyValue<K, V>) {
        (t, f(t))
    };

    let add_segment = |
        pq: &mut BinaryHeap<KeyValue<<K as Delta>::Output, ((f64, KeyValue<K, V>), (f64, KeyValue<K, V>))>>,
        low: (f64, KeyValue<K, V>),
        high: (f64, KeyValue<K, V>),
    | {
        // This requires the difference to be commutative.
        // delta between keys
        pq.push(KeyValue((&(high.1).0).delta(&(low.1).0), (low, high)));
    };

    let (t_min, t_max) = range.clone().into_inner();
    let (min, max) = (evaled_pair(t_min), evaled_pair(t_max));
    let mut ts = vec![(min.1).1.clone(), (max.1).1.clone()];

    add_segment(&mut pq, min, max);

    while (ts.len() as u64) < samples {
        // println!("loop");
        // Get the segment with the largest delta.
        let KeyValue(delta, (low, high)) = pq.pop().unwrap();
        // log(&format!("{:?}", delta));
        // Get the midpoint of the segment.
        let mid = evaled_pair(low.0 / 2.0 + high.0 / 2.0);
        ts.push((mid.1).1.clone());
        add_segment(&mut pq, low, mid.clone());
        add_segment(&mut pq, mid, high);
    }

    ts
}

#[derive(Clone, Copy)]
struct Point2D(f64, f64);

impl Delta for Point2D {
    type Output = OrdFloat;

    fn delta(&self, other: &Self) -> Self::Output {
        OrdFloat((self.0 - other.0).powf(2.0) + (self.1 - other.1).powf(2.0))
    }
}

#[derive(Clone)]
struct SpatialObjectWithPayload<S: SpatialObject, T>(S, T);

impl<S: SpatialObject, T> SpatialObject for SpatialObjectWithPayload<S, T> {
    type Point = <S as SpatialObject>::Point;

    fn mbr(&self) -> BoundingRect<Self::Point> {
        self.0.mbr()
    }

    fn distance2(&self, point: &Self::Point) -> <Self::Point as PointN>::Scalar {
        self.0.distance2(point)
    }
}

fn approximate_reflection_adaptive_rtree_quads(
    mirror: &Equation,
    figure: &Equation,
    interval: &Interval,
    _view: &View,
    thresh: f64,
) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
    let mut pairs = vec![];
    let norms = vec![];

    let range = interval.start..=interval.end;
    let samples = ((interval.end - interval.start) / interval.step) as u64 + 1;

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

    let samps1: Vec<_> = (Interval { start: -256.0, end: 256.0, step: 1.0 }).iter().map(|t| {
        let normal = mirror.normal(t);
        let samps: Vec<((f64, f64), (f64, f64), (f64, f64))> = (Interval { start: -256.0, end: 256.0, step: 512.0 }).iter().map(|s| {
            ((normal.function)(s), (normal.function)(-s), (t, s))
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
                    println!("add quad {:?} {:?} {:?} {:?}", ((s11.0).0, (s11.0).1), ((s12.0).0, (s12.0).1), ((s22.0).0, (s22.0).1), ((s21.0).0, (s21.0).1));
                    println!("{:?} {:?} {:?} {:?}", s11.2, s12.2, s22.2, s21.2);
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

    let figure_sample = adaptive_sample(
        |t| {
            // log(&format!("{:?}", t));
            let (x, y) = (figure.function)(t);
            KeyValue(Point2D(x, y), (x, y))
        },
        &range,
        samples * 2,
    );
    let interval_sample = figure.sample(&(Interval { start: -256.0, end: 256.0, step: 0.5 }));

    let threshold = thresh.sqrt();

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
        for SpatialObjectWithPayload(quad, (v1, v2, v3, v4)) in rtree.lookup_in_circle(p, &threshold) {


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

            // fn calc(p: &[f64; 2], q: &[f64; 2]) -> f64 {
            //     (quad.diam - p.sub(&q).length2().sqrt()) / quad.diam
            // }
            // // find distance of point to each of four vertices
            // let dis_s11 = calc(p, &quad.points[0]);
            // let dis_s12 = calc(p, &quad.points[1]);
            // let dis_s22 = calc(p, &quad.points[2]);
            // let dis_s21 = calc(p, &quad.points[3]);
            // let dis_sum = (dis_s11 + dis_s12 + dis_s22 + dis_s21) / (4 * quad.diam);
            // // make a weighted average of vs
            // // let weight = [
            // //     (v1.0 * dis_s11 + v2.0 * dis_s12 + v3.0 * dis_s22 + v4.0 * dis_s21) / dis_sum,
            // //     (v1.1 * dis_s11 + v2.1 * dis_s12 + v3.1 * dis_s22 + v4.1 * dis_s21) / dis_sum,
            // // ];
            // let weight = [(v1.0 + v2.0 + v3.0 + v4.0) / 4.0, (v1.1 + v2.1 + v3.1 + v4.1) / 4.0];
            // let [x, y] = weight;
            // reflection.insert((x.to_bits(), y.to_bits()));
        }
    }

    (reflection.iter().map(|(x, y)| (f64::from_bits(*x), f64::from_bits(*y))).collect(), norms)
}

fn approximate_reflection_adaptive_rtree_lines(
    mirror: &Equation,
    figure: &Equation,
    interval: &Interval,
    _view: &View,
    thresh: f64,
) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
    let mut pairs = vec![];
    let mut norms = vec![];

    let range = interval.start..=interval.end;
    let samples = ((interval.end - interval.start) / interval.step) as u64 + 1;

    for t in (Interval { start: -128.0, end: 128.0, step: 1.0 }).iter() {
        let normal = mirror.normal(t);
        let mut norm = vec![];
        // should be able to reduce sampling significantly here (only when linear)
        let samps: Vec<((f64, f64), (f64, f64))> = (Interval { start: -128.0, end: 128.0, step: 256.0 }).iter().map(|s| {
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

    let figure_sample = adaptive_sample(
        |t| {
            // log(&format!("{:?}", t));
            let (x, y) = (figure.function)(t);
            KeyValue(Point2D(x, y), (x, y))
        },
        &range,
        samples,
    );
    let interval_sample = figure.sample(&(Interval { start: -128.0, end: 128.0, step: 1.0 }));

    let threshold = thresh.sqrt();

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

fn approximate_reflection_adaptive(
    mirror: &Equation,
    figure: &Equation,
    interval: &Interval,
    _view: &View,
    thresh: f64,
) -> (Vec<(f64, f64)>, Vec<Vec<(f64, f64)>>) {
    let mut pairs = vec![];
    let mut norms = vec![];

    let range = interval.start..=interval.end;
    let samples = ((interval.end - interval.start) / interval.step) as u64 + 1;

    // break ties in gradient delta with distance
    // also optimise so don't split ranges that aren't changing (e.g. linear equations)
    // we're still doing too much repeated computation here

    let mirror_sample = adaptive_sample(
        |t| KeyValue(mirror.gradient(t), Rc::new(mirror.normal(t))),
        &range,
        samples,
    );

    for normal in mirror_sample {
        let points = adaptive_sample(
            |s| KeyValue(normal.gradient(s), ((normal.function)(s), (normal.function)(-s))),
            &range,
            samples,
        );
        let mut norm = vec![];
        for ((x, y), r) in points {
            norm.push((x, y));
            pairs.push(([x, y], r));
        }
        norms.push(norm);
    }

    let mut kdtree = KdTree::new_with_capacity(2, (samples * samples) as usize);
    for (p, (s, t)) in &pairs {
        kdtree.add(p, (s, t)).unwrap();
    }

    let mut reflection = HashSet::new();

    let figure_sample = adaptive_sample(
        |t| {
            let (x, y) = (figure.function)(t);
            KeyValue(Point2D(x, y), (x, y))
        },
        &range,
        samples,
    );

    for (x, y) in figure_sample {
        if let Ok(iter) = kdtree.iter_nearest(&[x, y], &squared_euclidean) {
            for (dist, point) in iter {
                if dist > thresh {
                    break;
                }
                let (x, y) = point;
                reflection.insert((x.to_bits(), y.to_bits()));
            }
        }
    }

    (reflection.iter().map(|(x, y)| (f64::from_bits(*x), f64::from_bits(*y))).collect(), norms)
}

fn parse_equation(string: String) -> Result<parser::Expr, ()> {
    if let Ok(lexemes) = Lexer::scan(string.chars().collect()) {
        let tokens = Lexer::evaluate(lexemes);
        let mut parser = Parser::new(tokens);
        parser.parse_equation()
    } else {
        Err(())
    }
}

fn construct_equation(string_x: String, string_y: String) -> Result<Equation, ()> {
    let expr_x = parse_equation(string_x)?;
    let expr_y = parse_equation(string_y)?;
    Ok(Equation {
        function: Box::new(move |t| {
            let mut bindings = HashMap::new();
            bindings.insert('t', t);
            (expr_x.evaluate(&bindings), expr_y.evaluate(&bindings))
        }),
    })
}

#[wasm_bindgen]
pub extern fn proof_of_concept(x: f64, y: f64, figure_x: String, figure_y: String, method: String, norms: bool, thresh: f64) -> String {
    let mirror = Equation {
        function: Box::new(|t| {
            let tx = t / 10.0;
            (t, tx * tx)
        })
    };
    let figure = if let Ok(figure) = construct_equation(figure_x.clone(), figure_y.clone()) {
        figure
    } else {
        log(&format!("error parsing figure {:?}", (figure_x, figure_y)));
        return String::new();
    };

    let interval = Interval { start: -256.0, end: 256.0, step: 0.5 };
    let width = 640.0;
    let height = 480.0;
    let pixels_per_cell = thresh / 10.0;
    let view = View {
        cols: (width / pixels_per_cell) as u16,
        rows: (height / pixels_per_cell) as u16,
        size: pixels_per_cell,
        x: x - width / 2.0,
        y: y - height / 2.0,
    };

    let (refl, mut normals) = match method.as_ref() {
        "visual" => approximate_reflection_vis(&mirror, &figure, &interval, &view, thresh),
        "kd" => approximate_reflection_kd(&mirror, &figure, &interval, &view, thresh),
        "lines" => approximate_reflection_adaptive_rtree_lines(&mirror, &figure, &interval, &view, thresh),
        "quads" =>  approximate_reflection_adaptive_rtree_quads(&mirror, &figure, &interval, &view, thresh),
        _ => panic!("unknown rendering method"),
    };

    if !norms {
        normals = vec![];
    }
    // let normals: Vec<_> = interval.iter().map(|t| mirror.normal(t).sample(&interval)).collect();
    // let normals: Vec<()> = vec![];
    // log(&format!("normals {:?}", normals));
    let json = json!((
        mirror.sample(&interval),
        normals,
        figure.sample(&interval),
        refl,
    ));
    json.to_string()
}
