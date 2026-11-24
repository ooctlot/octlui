//! GPU rendering tests. They need a Vulkan/Metal/DX12 adapter (lavapipe works in CI) and are
//! skipped when none is available.

use octlui::geometry::*;
use octlui::headless::HeadlessRenderer;
use octlui::style::*;
use octlui::*;

fn renderer(w: u32, h: u32) -> Option<HeadlessRenderer> {
    match HeadlessRenderer::new(w, h, 1.0) {
        Ok(mut r) => {
            r.set_clear_color(Color::BLACK);
            Some(r)
        }
        Err(e) => {
            eprintln!("skipping GPU test: {e}");
            None
        }
    }
}

fn close(a: [u8; 4], b: [u8; 3], tol: i32) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= tol)
}

#[test]
fn solid_rect_and_clear_color() {
    let Some(mut r) = renderer(64, 64) else { return };
    let shot = r.render(node().size(fixed(32.0), fixed(32.0)).background(0xff0000));
    assert!(close(shot.pixel(16, 16), [255, 0, 0], 1));
    assert!(close(shot.pixel(48, 48), [0, 0, 0], 0));
}

#[test]
fn corner_radius_cuts_corners() {
    let Some(mut r) = renderer(64, 64) else { return };
    let shot = r.render(node().size(fixed(64.0), fixed(64.0)).corner_radius(20.0).background(0xffffff));
    assert!(close(shot.pixel(0, 0), [0, 0, 0], 2), "corner pixel is outside the rounded shape");
    assert!(close(shot.pixel(32, 32), [255, 255, 255], 1));
    assert!(close(shot.pixel(32, 0), [255, 255, 255], 2), "edge middle is covered");
}

#[test]
fn per_corner_radius() {
    let Some(mut r) = renderer(64, 64) else { return };
    let shot = r.render(node().size(fixed(64.0), fixed(64.0)).corner_radius([30.0, 0.0, 0.0, 0.0]).background(0xffffff));
    assert!(close(shot.pixel(0, 0), [0, 0, 0], 2), "top-left is rounded");
    assert!(close(shot.pixel(63, 0), [255, 255, 255], 2), "top-right stays square");
    assert!(close(shot.pixel(63, 63), [255, 255, 255], 2));
}

#[test]
fn border_is_drawn_inside() {
    let Some(mut r) = renderer(64, 64) else { return };
    let shot = r.render(node().size(fixed(64.0), fixed(64.0)).background(0x0000ff).border(0x00ff00, 4.0));
    assert!(close(shot.pixel(1, 32), [0, 255, 0], 2));
    assert!(close(shot.pixel(32, 32), [0, 0, 255], 1));
}

#[test]
fn linear_gradient_goes_left_to_right() {
    let Some(mut r) = renderer(100, 20) else { return };
    let shot = r.render(
        node().size(fixed(100.0), fixed(20.0)).background(Fill::linear(0.0, &[Color::hex(0xff0000), Color::hex(0x0000ff)])),
    );
    let (l, m, rr) = (shot.pixel(2, 10), shot.pixel(50, 10), shot.pixel(97, 10));
    assert!(l[0] > 240 && l[2] < 15);
    assert!(rr[2] > 240 && rr[0] < 15);
    assert!((m[0] as i32 - 128).abs() < 12 && (m[2] as i32 - 128).abs() < 12);
}

#[test]
fn clip_hides_overflow() {
    let Some(mut r) = renderer(100, 100) else { return };
    let shot = r.render(
        node()
            .size(fixed(50.0), fixed(50.0))
            .clip(Clip::XY)
            .child(node().size(fixed(100.0), fixed(100.0)).background(0xffffff)),
    );
    assert!(close(shot.pixel(25, 25), [255, 255, 255], 1));
    assert!(close(shot.pixel(75, 75), [0, 0, 0], 0), "clipped away");
    assert!(close(shot.pixel(75, 25), [0, 0, 0], 0));
}

#[test]
fn image_is_sampled_and_tinted() {
    let Some(mut r) = renderer(64, 64) else { return };
    let id = r.images().insert_rgba(2, 2, [255u8, 255, 255, 255].repeat(4));
    let shot = r.render(
        node()
            .size(fixed(64.0), fixed(64.0))
            .image(components::Image::new(id).tint(0xff8000).fit(ImageFit::Fill)),
    );
    assert!(close(shot.pixel(32, 32), [255, 128, 0], 2));
}

#[test]
fn alpha_blending_over_background() {
    let Some(mut r) = renderer(32, 32) else { return };
    let shot = r.render(
        node()
            .size(fixed(32.0), fixed(32.0))
            .background(0x0000ff)
            .child(node().size(fixed(32.0), fixed(32.0)).background(Color::rgba(1.0, 0.0, 0.0, 0.5))),
    );
    // 50% red over blue.
    assert!(close(shot.pixel(16, 16), [128, 0, 128], 3), "{:?}", shot.pixel(16, 16));
}

#[test]
fn many_quads_render() {
    let Some(mut r) = renderer(200, 200) else { return };
    // Exercises instance-buffer growth well past the initial capacity.
    let cells = (0..4000).map(|i| {
        node().size(fixed(2.0), fixed(2.0)).background(if i % 2 == 0 { 0xffffff } else { 0x000000 })
    });
    let shot = r.render(node().size(fixed(200.0), fixed(200.0)).row().children(cells));
    assert!(close(shot.pixel(0, 0), [255, 255, 255], 1));
}

#[test]
fn cached_native_glyphs_draw_inside_text_clip() {
    let Some(mut r) = renderer(96, 40) else { return };
    let shot = r.render(node().size(fixed(48.), fixed(40.)).text(
        components::Text::new("MMMMMMMM").size(24.).color(0xffffff).wrap(TextWrap::Clip { ellipsis: false })
    ));
    assert!((0..40).any(|y| (0..48).any(|x| shot.pixel(x,y)[0] > 128)));
    assert!((0..40).all(|y| (48..96).all(|x| shot.pixel(x,y)[0] == 0)));
    assert_eq!(shot.rgba, r.read_pixels().rgba);
}

#[test]
fn zero_width_fraction_has_no_antialiasing_fringe() {
    let Some(mut r) = renderer(401, 40) else { return };
    let track = || node().size(fixed(400.5), fixed(24.0)).background(0x313244).clip(Clip::X);
    let root = || node().size(grow(), grow()).center();
    let empty = r.render(root().child(track()));
    let with_fill = r.render(root().child(track().child(
        node()
            .width(fraction(0.0))
            .height(grow())
            .background(0xffffff)
            .spring(components::Spring::new(components::SpringProps::SIZE)),
    )));
    // The left edge is at x = 0.25, within the AA fringe of the first pixel center.
    assert!(with_fill.rgba == empty.rgba, "a zero-width fill changed the track pixels");
}
