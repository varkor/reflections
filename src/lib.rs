#![feature(box_syntax)]
#![feature(euclidean_division)]
#![feature(self_struct_ctor)]
#![feature(try_trait)]

#![deny(bare_trait_objects)]

extern crate console_error_panic_hook;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate spade;
extern crate wasm_bindgen;

pub mod approximation;
pub mod parser;
pub mod reflectors;
pub mod sampling;
pub mod spatial;

use std::collections::HashMap;

use wasm_bindgen::prelude::wasm_bindgen;

pub use approximation::Equation;
use approximation::{Interval, View};
use parser::{Lexer, Parser};
pub use reflectors::{RasterisationApproximator, LinearApproximator, QuadraticApproximator};
use reflectors::ReflectionApproximator;
use spatial::Point2D;

// It's helpful to be able to log error messages to the JavaScript console, so we export some
// methods to do so here.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(s: &str);
}

/// JavaScript `console.log`.
#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => (console_log(&format_args!($($t)*).to_string()))
}

/// Construct a parametric equation given the strings corresponding to `x(t)` and `y(t)`.
fn construct_equation<'a>(string_x: String, string_y: String) -> Result<Equation<'a>, ()> {
    /// Convert a string into an expression, which can then be evaluated to create an equation.
    fn parse_equation(string: String) -> Result<parser::Expr, ()> {
        if let Ok(lexemes) = Lexer::scan(string.chars()) {
            let tokens = Lexer::evaluate(lexemes.into_iter()).collect();
            let mut parser = Parser::new(tokens);
            parser.parse()
        } else {
            Err(())
        }
    }

    let expr = [parse_equation(string_x)?, parse_equation(string_y)?];
    Ok(Equation {
        function: box move |t| {
            let mut bindings = HashMap::new();
            bindings.insert('t', t);
            Point2D::new([expr[0].evaluate(&bindings), expr[1].evaluate(&bindings)])
        },
    })
}

/// Set up the Rust WASM environment. Responsible primarily for setting up the error handlers.
#[wasm_bindgen]
pub extern fn initialise() {
    console_error_panic_hook::set_once();
}

/// Approximate a generalised reflection given a mirror and figure, as a set of points.
#[wasm_bindgen]
pub extern fn render_reflection(
    x: f64,
    y: f64,
    width: u16,
    height: u16,
    zoom: f64,
    figure_x: String,
    figure_y: String,
    mirror_x: String,
    mirror_y: String,
    method: String,
    threshold: f64,
    scale: f64,
    translate: f64
) -> String {
    let (figure, mirror) = match (
        construct_equation(figure_x, figure_y),
        construct_equation(mirror_x, mirror_y),
    ) {
        (Ok(figure), Ok(mirror)) => (figure, mirror),
        _ => return String::new(),
    };

    let interval = Interval { start: -256.0, end: 256.0, step: 1.0 };
    let view = View {
        width,
        height,
        origin: Point2D::new([x, y]),
        size: Point2D::new([width as f64 * 2.0f64.powf(zoom), height as f64 * 2.0f64.powf(zoom)]),
    };

    let reflection = match method.as_ref() {
        "rasterisation" => {
            let approximator = RasterisationApproximator {
                cell_size: (threshold as u16).max(1),
            };
            approximator.approximate_reflection(&mirror, &figure, &interval, &view, scale, translate)
        }
        "linear" => {
            let approximator = LinearApproximator { threshold };
            approximator.approximate_reflection(&mirror, &figure, &interval, &view, scale, translate)
        }
        "quadratic" => {
            let approximator = QuadraticApproximator;
            approximator.approximate_reflection(&mirror, &figure, &interval, &view, scale, translate)
        }
        _ => panic!("unknown rendering method"),
    };

    json!((
        mirror.sample(&interval),
        figure.sample(&interval),
        reflection,
    )).to_string()
}
