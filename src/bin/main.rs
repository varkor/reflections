#![allow(unused_imports)]

extern crate reflections;

use reflections::parser::Lexer;
use reflections::proof_of_concept;

fn main() {
    proof_of_concept(0.0, 0.0, "t".to_string(), "0".to_string(), "t".to_string(), "(100/1200)*t*t".to_string(), "quads".to_string(), false, 2.0);
}
