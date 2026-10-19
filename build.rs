//! Embed the same ordered Metal sources used by the standalone shader checker.
use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let shaders = Path::new("src/tracer/metal/shaders");
    let manifest = shaders.join("sources.txt");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let mut source = String::new();
    for name in fs::read_to_string(&manifest)?
        .lines()
        .filter(|name| !name.is_empty())
    {
        let path = shaders.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        source.push_str(&fs::read_to_string(path)?);
        source.push('\n');
    }
    fs::write(
        Path::new(&env::var("OUT_DIR")?).join("rustytracer.metal"),
        source,
    )?;
    Ok(())
}
