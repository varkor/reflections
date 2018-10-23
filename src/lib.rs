#![feature(try_trait)]
#![feature(self_struct_ctor)]
#![feature(euclidean_division)]
#![feature(box_syntax)]

#![deny(bare_trait_objects)]

extern crate wasm_bindgen;
extern crate spade;
#[macro_use]
extern crate serde_json;
extern crate console_error_panic_hook;

use wasm_bindgen::prelude::wasm_bindgen;

use std::collections::HashMap;

pub mod parser;
use parser::{Lexer, Parser};

pub mod approximation;
use approximation::{Interval, View};
pub use approximation::OrdFloat;
pub use approximation::Equation;
use approximators::ReflectionApproximator;
pub use approximation::adaptive_sample;
pub use approximation::KeyValue;

pub mod approximators;
pub use approximators::{RasterisationApproximator, LinearApproximator, QuadraticApproximator};

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
            (expr_x.evaluate(&bindings), expr_y.evaluate(&bindings))
        }),
    })
}

#[wasm_bindgen]
pub extern fn proof_of_concept(x: f64, y: f64, figure_x: String, figure_y: String, mirror_x: String, mirror_y: String, method: String, norms: bool, thresh: f64) -> String {
    console_error_panic_hook::set_once();

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
        "rasterisation" => {
            let approximator = RasterisationApproximator;
            approximator.approximate_reflection(&mirror, &figure, &interval, &view)
        }
        "linear" => {
            let approximator = LinearApproximator(thresh);
            approximator.approximate_reflection(&mirror, &figure, &interval, &view)
        }
        "quadratic" => {
            let approximator = QuadraticApproximator;
            approximator.approximate_reflection(&mirror, &figure, &interval, &view)
        }
        _ => panic!("unknown rendering method"),
    };

    if !norms {
        normals = vec![];
    }
    // let normals: Vec<_> = interval.iter().map(|t| mirror.normal(t).sample(&Interval { start: -256.0, end: 256.0, step: 2.0 }))).collect();
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
