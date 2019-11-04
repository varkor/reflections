#![feature(bind_by_move_pattern_guards)]
#![feature(box_syntax)]
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
    static_bindings: &'a HashMap<char, f64>,
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
            Point2D::new([
                expr[0].evaluate((&bindings, static_bindings)),
                expr[1].evaluate((&bindings, static_bindings)),
            ])
        },
    })
}

/// A variable binding: a name and value, along with the range of values the variable can take.
///
/// The struct `Binding` mirrors the JavaScript class `Binding` and should be kept in sync.
#[derive(Clone, Debug, Deserialize)]
struct Binding {
    value: f64,
    min: f64,
    max: f64,
    step: f64,
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
        bindings: HashMap<&'a str, Binding>,
        method: &'a str,
        threshold: f64,
    }

    /// The struct `RenderReflectionData` mirrors the JavaScript class `RenderReflectionData` and
    /// should be kept in sync.
    #[derive(Serialize)]
    struct RenderReflectionData {
        mirror: Vec<Point2D>,
        figure: Vec<Point2D>,
        reflection: Vec<(Point2D, Point2D, Point2D)>,
    }

    // An empty string represents an error to the JavaScript client.
    let error_output = String::new();

    if let Ok(data) = serde_json::from_str::<RenderReflectionArgs>(&json) {
        // `t` and `s` are inherently special-cased. We use their values as offset parameters.
        let (s_offset, t_offset) = (data.bindings["s"].value, data.bindings["t"].value);
        let bindings: HashMap<char, f64> = data.bindings.iter().filter_map(|(name, binding)| {
            match (name.len(), name) {
                (_, &"s") | (_, &"t") => None,
                (1, _) => name.chars().next().map(|c| (c, binding.value)),
                _ => None,
            }
        }).collect();

        let (figure, mirror, sigma_tau) = match (
            construct_equation(data.figure, &bindings, |bindings, t| {
                bindings.insert('t', t);
            }),
            construct_equation(data.mirror, &bindings, |bindings, t| {
                bindings.insert('t', t);
            }),
            construct_equation(data.sigma_tau, &bindings, |bindings, (s, t)| {
                bindings.insert('s', s - s_offset);
                bindings.insert('t', t - t_offset);
            }),
        ) {
            (Ok(figure), Ok(mirror), Ok(sigma_tau)) => (figure, mirror, sigma_tau),
            _ => return error_output,
        };

        // The interval over which to sample `t`.
        // For now, we use the same interval for sampling `s`, to simplify the interface.
        let interval = Interval {
            start: data.bindings["t"].min,
            end: data.bindings["t"].max,
            step: data.bindings["t"].step,
        };

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
