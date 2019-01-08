mod tracer;
use crate::tracer::*;

fn main() {
    generate("image.png", 800,600).expect("Error, failure to write to file.");
}
