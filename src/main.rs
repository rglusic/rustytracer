mod tracer;
use crate::tracer::*;

fn main() {
    generate("output/image.png", 200,100).expect("Error, failure to write to file.");
}
