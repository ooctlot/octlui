//! Renders the showcase offscreen to `showcase.ppm`: `cargo run --example headless`.
#[path = "showcase.rs"]
#[allow(dead_code)]
mod showcase;

fn main() {
    let mut r = octlui::headless::HeadlessRenderer::new(960, 640, 1.0).expect("GPU");
    println!("adapter: {}", r.adapter_name());
    r.set_root(showcase::showcase());
    r.frame(0.016);
    let shot = r.read_pixels();
    std::fs::write("showcase.ppm", shot.to_ppm()).unwrap();
    println!("wrote showcase.ppm ({}x{})", shot.width, shot.height);
}
