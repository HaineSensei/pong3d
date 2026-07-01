use crate::geometry::VertexHud;

/// Converts a pixel-space rectangle (origin top-left, y-down) into two
/// screen-space triangles in clip space (NDC, y-up) and appends them.
fn push_rect_px(
    out: &mut Vec<VertexHud>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let to_ndc = |px: f32, py: f32| -> [f32; 2] {
        [(px / screen_w) * 2.0 - 1.0, 1.0 - (py / screen_h) * 2.0]
    };
    let a = to_ndc(x, y);
    let b = to_ndc(x + w, y);
    let c = to_ndc(x + w, y + h);
    let d = to_ndc(x, y + h);
    for p in [a, b, c, a, c, d] {
        out.push(VertexHud { position: p, color });
    }
}

/// Renders a single digit (0-9) as a blocky seven-segment glyph made of
/// rectangles, at pixel position (x, y) with glyph size (w, h).
fn push_digit(
    out: &mut Vec<VertexHud>,
    ch: char,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let t = (w * 0.22).max(2.0); // segment thickness
    // segment order: top, top-right, bottom-right, bottom, bottom-left, top-left, middle
    let segs: [bool; 7] = match ch {
        '0' => [true, true, true, true, true, true, false],
        '1' => [false, true, true, false, false, false, false],
        '2' => [true, true, false, true, true, false, true],
        '3' => [true, true, true, true, false, false, true],
        '4' => [false, true, true, false, false, true, true],
        '5' => [true, false, true, true, false, true, true],
        '6' => [true, false, true, true, true, true, true],
        '7' => [true, true, true, false, false, false, false],
        '8' => [true, true, true, true, true, true, true],
        '9' => [true, true, true, true, false, true, true],
        _ => [false; 7],
    };
    if segs[0] {
        push_rect_px(out, x, y, w, t, screen_w, screen_h, color);
    }
    if segs[1] {
        push_rect_px(out, x + w - t, y, t, h / 2.0, screen_w, screen_h, color);
    }
    if segs[2] {
        push_rect_px(out, x + w - t, y + h / 2.0, t, h / 2.0, screen_w, screen_h, color);
    }
    if segs[3] {
        push_rect_px(out, x, y + h - t, w, t, screen_w, screen_h, color);
    }
    if segs[4] {
        push_rect_px(out, x, y + h / 2.0, t, h / 2.0, screen_w, screen_h, color);
    }
    if segs[5] {
        push_rect_px(out, x, y, t, h / 2.0, screen_w, screen_h, color);
    }
    if segs[6] {
        push_rect_px(out, x, y + h / 2.0 - t / 2.0, w, t, screen_w, screen_h, color);
    }
}

/// Renders a non-negative integer score at pixel position (x, y), growing
/// rightward.
pub fn push_score(
    out: &mut Vec<VertexHud>,
    score: u32,
    x: f32,
    y: f32,
    screen_w: f32,
    screen_h: f32,
    color: [f32; 4],
) {
    let digit_w = 26.0;
    let digit_h = 40.0;
    let gap = 10.0;
    let s = score.to_string();
    for (i, ch) in s.chars().enumerate() {
        let dx = x + i as f32 * (digit_w + gap);
        push_digit(out, ch, dx, y, digit_w, digit_h, screen_w, screen_h, color);
    }
}

/// Small crosshair at screen center to help aim the FPS-style view.
pub fn push_crosshair(out: &mut Vec<VertexHud>, screen_w: f32, screen_h: f32, color: [f32; 4]) {
    let cx = screen_w / 2.0;
    let cy = screen_h / 2.0;
    let len = 10.0;
    let t = 2.0;
    push_rect_px(out, cx - len, cy - t / 2.0, len * 2.0, t, screen_w, screen_h, color);
    push_rect_px(out, cx - t / 2.0, cy - len, t, len * 2.0, screen_w, screen_h, color);
}

/// A thin colored bar across the top of the screen, used as a serve/turn
/// indicator or subtle status strip.
#[allow(dead_code)]
pub fn push_top_bar(out: &mut Vec<VertexHud>, screen_w: f32, screen_h: f32, color: [f32; 4]) {
    push_rect_px(out, 0.0, 0.0, screen_w, 6.0, screen_w, screen_h, color);
}

/// Full-screen translucent tint, used for the win/loss flash.
pub fn push_full_screen_tint(out: &mut Vec<VertexHud>, screen_w: f32, screen_h: f32, color: [f32; 4]) {
    push_rect_px(out, 0.0, 0.0, screen_w, screen_h, screen_w, screen_h, color);
}
