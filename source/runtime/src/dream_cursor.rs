//! Small scanout-visible cursor, independent of KMS hardware cursor planes.
use glow::HasContext;

pub unsafe fn draw(gl: &glow::Context, x: i32, y: i32, width: i32, height: i32) {
    let mut old_box = [0; 4];
    let mut old_color = [0.0; 4];
    let mut old_mask = [0; 4];
    let scissor = gl.is_enabled(glow::SCISSOR_TEST);
    gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut old_box);
    gl.get_parameter_f32_slice(glow::COLOR_CLEAR_VALUE, &mut old_color);
    gl.get_parameter_i32_slice(glow::COLOR_WRITEMASK, &mut old_mask);
    gl.enable(glow::SCISSOR_TEST);
    gl.color_mask(true, true, true, true);
    // An outlined white arrow. Clearing narrow scissored spans needs no shader,
    // texture, blend or vertex-state changes and leaves game pixel data intact.
    // Batch by color: preserve the exact original arrow pixels, but set
    // glClearColor only twice rather than for every outlined scanline.
    for inner in [false, true] {
        let color = if inner { 1.0 } else { 0.0 };
        gl.clear_color(color, color, color, 1.0);
        for row in 0..24 {
            let (left, right) = if row < 17 { (0, row * 2 / 3 + 1) } else { (5, 10) };
            let px = x + left;
            let py = height - 1 - y - row;
            if px >= width || py < 0 || py >= height { continue; }
            let n = (right-left).min(width-px);
            if n <= 0 { continue; }
            if inner {
                if row == 0 || row >= 23 || n <= 2 { continue; }
                gl.scissor(px+1, py, n-2, 1);
            } else {
                gl.scissor(px, py, n, 1);
            }
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
    }
    gl.clear_color(old_color[0],old_color[1],old_color[2],old_color[3]);
    gl.color_mask(old_mask[0]!=0,old_mask[1]!=0,old_mask[2]!=0,old_mask[3]!=0);
    gl.scissor(old_box[0],old_box[1],old_box[2],old_box[3]);
    if !scissor { gl.disable(glow::SCISSOR_TEST); }
}
