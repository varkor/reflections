#![feature(box_syntax)]
#![feature(euclidean_division)]
#![feature(try_trait)]

#![deny(bare_trait_objects)]

use console_error_panic_hook;

#[macro_use] extern crate serde_derive;
#[macro_use] extern crate serde_json;

pub mod approximation;
pub mod parser;
pub mod reflectors;
// We don't actually make use of `sampling` yet, but we'd like to make sure it continues to compile.
pub mod sampling;
pub mod spatial;

use std::collections::HashMap;

use wasm_bindgen::prelude::wasm_bindgen;

use crate::approximation::Equation;
use crate::approximation::{Interval, View};
use crate::parser::{Lexer, Parser};
use crate::reflectors::{RasterisationApproximator, LinearApproximator, QuadraticApproximator};
use crate::reflectors::ReflectionApproximator;
use crate::spatial::Point2D;

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
fn construct_equation<'a, I>(
    string: [&str; 2],
    set_bindings: impl 'a + Fn(&mut HashMap<char, f64>, I),
) -> Result<Equation<'a, I>, ()> {
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
        function: box move |p| {
            let mut bindings = HashMap::new();
            set_bindings(&mut bindings, p);
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
    /// The struct `RenderReflectionArgs` mirrors the JavaScript class `RenderReflectionArgs` and
    /// should be kept in sync.
    #[derive(Deserialize)]
    struct RenderReflectionArgs<'a> {
        view: View,
        mirror: [&'a str; 2],
        figure: [&'a str; 2],
        sigma_tau: [&'a str; 2],
        method: &'a str,
        threshold: f64,
    }

    /// The struct `RenderReflectionData` mirrors the JavaScript class `RenderReflectionData` and
    /// should be kept in sync.
    #[derive(Serialize)]
    struct RenderReflectionData {
        mirror: Vec<Point2D>,
        figure: Vec<Point2D>,
        reflection: Vec<Point2D>,
    }

    // An empty string represents an error to the JavaScript client.
    let error_output = String::new();

    if let Ok(data) = serde_json::from_str::<RenderReflectionArgs>(&json) {
        let (figure, mirror, sigma_tau) = match (
            construct_equation(data.figure, |bindings, t| { bindings.insert('t', t); }),
            construct_equation(data.mirror, |bindings, t| { bindings.insert('t', t); }),
            construct_equation(data.sigma_tau, |bindings, (s, t)| {
                bindings.insert('s', s);
                bindings.insert('t', t);
            }),
        ) {
            (Ok(figure), Ok(mirror), Ok(sigma_tau)) => (figure, mirror, sigma_tau),
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
                    &sigma_tau,
                    &interval,
                    &data.view,
                )
            }
            "linear" => {
                let approximator = LinearApproximator { threshold: data.threshold };
                approximator.approximate_reflection(
                    &mirror,
                    &figure,
                    &sigma_tau,
                    &interval,
                    &data.view,
                )
            }
            "quadratic" => {
                let approximator = QuadraticApproximator;
                approximator.approximate_reflection(
                    &mirror,
                    &figure,
                    &sigma_tau,
                    &interval,
                    &data.view,
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
