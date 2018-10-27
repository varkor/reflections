#![feature(try_trait)]
#![feature(self_struct_ctor)]
#![feature(euclidean_division)]
#![feature(box_syntax)]

#![deny(bare_trait_objects)]

extern crate wasm_bindgen;
extern crate spade;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate console_error_panic_hook;

use wasm_bindgen::prelude::wasm_bindgen;

use std::collections::HashMap;

pub mod parser;
use parser::{Lexer, Parser};

pub mod spatial;

use spatial::Point2D;

pub mod approximation;
use approximation::{Interval, View};
// pub use approximation::OrdFloat;
pub use approximation::Equation;
use reflectors::ReflectionApproximator;
// pub use approximation::adaptive_sample;
// pub use approximation::KeyValue;

pub mod reflectors;
pub use reflectors::{RasterisationApproximator, LinearApproximator, QuadraticApproximator};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (console_log(&format_args!($($t)*).to_string()))
}

fn parse_equation(string: String) -> Result<parser::Expr, ()> {
    if let Ok(lexemes) = Lexer::scan(string.chars()) {
        let tokens = Lexer::evaluate(lexemes.into_iter()).collect();
        let mut parser = Parser::new(tokens);
        parser.parse()
    } else {
        Err(())
    }
}

fn construct_equation<'a>(string_x: String, string_y: String) -> Result<Equation<'a>, ()> {
    let expr_x = parse_equation(string_x)?;
    let expr_y = parse_equation(string_y)?;
    Ok(Equation {
        function: Box::new(move |t| {
            let mut bindings = HashMap::new();
            bindings.insert('t', t);
            Point2D::new([expr_x.evaluate(&bindings), expr_y.evaluate(&bindings)])
        }),
    })
}

/// Set up the Rust WASM environment. Responsible primarily for setting up the error handlers.
#[wasm_bindgen]
pub extern fn initialise() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub extern fn proof_of_concept(x: f64, y: f64, figure_x: String, figure_y: String, mirror_x: String, mirror_y: String, method: String, _norms: bool, thresh: f64, scale: f64, translate: f64) -> String {
    let figure = if let Ok(figure) = construct_equation(figure_x.clone(), figure_y.clone()) {
        figure
    } else {
        console_log!("error parsing figure {:?}", (figure_x, figure_y));
        return String::new();
    };
    let mirror = if let Ok(mirror) = construct_equation(mirror_x.clone(), mirror_y.clone()) {
        mirror
    } else {
        console_log!("error parsing mirror {:?}", (mirror_x, mirror_y));
        return String::new();
    };

    let interval = Interval { start: -256.0, end: 256.0, step: 0.5 };
    let width = 640;
    let height = 480;
    let pixels_per_cell = ((thresh / 10.0) as u16).max(1);
    console_log!("pixels-per_cell {} {}", thresh, pixels_per_cell);
    let view = View {
        width,
        height,
        origin: Point2D::new([x, y]),
        size: Point2D::new([width as f64 * 2.0f64.powf(1.0), height as f64 * 2.0f64.powf(1.0)]), // FIXME: correct this
    };

    let refl = match method.as_ref() {
        "rasterisation" => {
            let approximator = RasterisationApproximator {
                cell_size: pixels_per_cell,
            };
            approximator.approximate_reflection(&mirror, &figure, &interval, &view, scale, translate)
        }
        "linear" => {
            let approximator = LinearApproximator(thresh);
            approximator.approximate_reflection(&mirror, &figure, &interval, &view, scale, translate)
        }
        "quadratic" => {
            let approximator = QuadraticApproximator;
            approximator.approximate_reflection(&mirror, &figure, &interval, &view, scale, translate)
        }
        _ => panic!("unknown rendering method"),
    };

    // let normals: Vec<_> = interval.clone().map(|t| mirror.normal(t).sample(&Interval { start: -256.0, end: 256.0, step: 2.0 }))).collect();
    let normals: Vec<()> = vec![];
    // log(&format!("normals {:?}", normals));
    let json = json!((
        mirror.sample(&interval),
        normals,
        figure.sample(&interval),
        refl,
    ));
    json.to_string()
}
