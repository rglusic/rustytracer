mod tracer;
use crate::tracer::*;

fn main() {
    generate_jpg(800, 600).expect("Error, failure to write to file.");
}
