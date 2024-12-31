use crate::{Colorful, Screen};

fn plot_low(screen: &mut Screen, x0: i32, y0: i32, x1: i32, y1: i32, color: &impl Colorful) {
    let dx = x1 - x0;
    let mut dy = y1 - y0;
    let mut yi = 1;
    if dy < 0 {
        yi = -1;
        dy = -dy;
    }
    let mut d = (2 * dy) - dx;
    let mut y = y0;

    for x in x0..x1 {
        screen.blend_px(x as usize, y as usize, color);
        if d > 0 {
            y += yi;
            d += 2 * (dy - dx);
        } else {
            d += 2 * dy;
        }
    }
}

fn plot_high(screen: &mut Screen, x0: i32, y0: i32, x1: i32, y1: i32, color: &impl Colorful) {
    let mut dx = x1 - x0;
    let dy = y1 - y0;
    let mut xi = 1;
    if dx < 0 {
        xi = -1;
        dx = -dx;
    }
    let mut d = (2 * dx) - dy;
    let mut x = x0;

    for y in y0..y1 {
        screen.blend_px(x as usize, y as usize, color);
        if d > 0 {
            x += xi;
            d += 2 * (dx - dy);
        } else {
            d += 2 * dx;
        }
    }
}

pub fn draw_line(screen: &mut Screen, x0: i32, y0: i32, x1: i32, y1: i32, color: &impl Colorful) {
    if (y1 - y0).abs() < (x1 - x0).abs() {
        if x0 > x1 {
            plot_low(screen, x1, y1, x0, y0, color);
        } else {
            plot_low(screen, x0, y0, x1, y1, color);
        }
    } else {
        if y0 > y1 {
            plot_high(screen, x1, y1, x0, y0, color);
        } else {
            plot_high(screen, x0, y0, x1, y1, color);
        }
    }
}
