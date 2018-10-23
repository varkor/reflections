#![allow(unused_imports)]

extern crate reflections;

use reflections::parser::Lexer;
use reflections::proof_of_concept;
// use reflections::adaptive_sample;
// use reflections::KeyValue;
// use reflections::OrdFloat;

fn main() {
    proof_of_concept(0.0, 0.0, "t".to_string(), "60".to_string(), "t".to_string(), "(t/10)^2".to_string(), "quads".to_string(), false, 2.0, true, 0.0);
    // let mut samps = adaptive_sample(|t| KeyValue((), t), &(-256.0..=256.0), 513);
    // samps.sort_unstable_by_key(|&x| OrdFloat(x));
    // println!("{:?}", samps);
}
