#![feature(box_syntax)]
#![feature(euclidean_division)]
#![feature(try_trait)]

#![deny(bare_trait_objects)]

extern crate console_error_panic_hook;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;
extern crate spade;
extern crate wasm_bindgen;

pub mod approximation;
pub mod parser;
pub mod reflectors;
// We don't actually make use of `sampling` yet, but we'd like to make sure it continues to compile.
pub mod sampling;
pub mod spatial;

use std::collections::HashMap;

use wasm_bindgen::prelude::wasm_bindgen;

use approximation::Equation;
use approximation::{Interval, View};
use parser::{Lexer, Parser};
use reflectors::{RasterisationApproximator, LinearApproximator, QuadraticApproximator};
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
fn construct_equation<'a>(string: [&str; 2]) -> Result<Equation<'a>, ()> {
    /// Convert a string into an expression, which can then be evaluated to create an equation.
    fn parse_equation(string: &str) -> Result<parser::Expr, ()> {
        if let Ok(lexemes) = Lexer::scan(string.chars()) {
            let tokens = Lexer::evaluate(lexemes.into_iter()).collect();
            let mut parser = Parser::new(tokens);
            parser.parse()
        } else {
            Err(())
        }
    }

    let expr = [parse_equation(string[0])?, parse_equation(string[1])?];
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
    json: String,
) -> String {
    /// The struct `RenderReflectionArgs` mirrors the JavaScript class `RenderReflectionArgs` and should
    /// be kept in sync.
    #[derive(Deserialize)]
    struct RenderReflectionArgs<'a> {
        view: View,
        mirror: [&'a str; 2],
        figure: [&'a str; 2],
        method: &'a str,
        threshold: f64,
        scale: f64,
        translate: f64,
    }

    /// The struct `RenderReflectionData` mirrors the JavaScript class `RenderReflectionData` and should
    /// be kept in sync.
    #[derive(Serialize)]
    struct RenderReflectionData {
        mirror: Vec<Point2D>,
        figure: Vec<Point2D>,
        reflection: Vec<Point2D>,
    }

    // An empty string represents an error to the JavaScript client.
    let error_output = String::new();

    if let Ok(data) = serde_json::from_str::<RenderReflectionArgs>(&json) {
        let (figure, mirror) = match (
            construct_equation(data.figure),
            construct_equation(data.mirror),
        ) {
            (Ok(figure), Ok(mirror)) => (figure, mirror),
            _ => return error_output,
        };

        // Currently we used a fix interval over which to sample `t`, but in the future this will
        // be more flexible.
        let interval = Interval { start: -256.0, end: 256.0, step: 1.0 };

        let reflection = match data.method.as_ref() {
            "rasterisation" => {
                let approximator = RasterisationApproximator {
                    cell_size: (data.threshold as u16).max(1),
                };
                approximator.approximate_reflection(
                    &mirror,
                    &figure,
                    &interval,
                    &data.view,
                    data.scale,
                    data.translate,
                )
            }
            "linear" => {
                let approximator = LinearApproximator { threshold: data.threshold };
                approximator.approximate_reflection(
                    &mirror,
                    &figure,
                    &interval,
                    &data.view,
                    data.scale,
                    data.translate,
                )
            }
            "quadratic" => {
                let approximator = QuadraticApproximator;
                approximator.approximate_reflection(
                    &mirror,
                    &figure,
                    &interval,
                    &data.view,
                    data.scale,
                    data.translate,
                )
            }
            _ => panic!("unknown rendering method"),
        };

        json!(RenderReflectionData {
            mirror: mirror.sample(&interval),
            figure: figure.sample(&interval),
            reflection,
        }).to_string()
    } else {
        error_output
    }
}
