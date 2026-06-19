//! Backend-independent software rasterization of the shared compositor scene.

use crate::render::{
    RenderCircle, RenderCommand, RenderImage, RenderRect, RenderRoundedRect, RenderSceneFrame,
    RenderText,
};

pub fn draw_scene(buffer: &mut [u32], width: u32, height: u32, frame: &RenderSceneFrame) {
    let required = width as usize * height as usize;
    if buffer.len() < required {
        return;
    }

    buffer[..required].fill(frame.clear_color.to_argb_u32());
    for command in &frame.commands {
        match command {
            RenderCommand::RoundedRect(rectangle) => {
                draw_rounded_rectangle(buffer, width, height, *rectangle);
            }
            RenderCommand::Rect(rectangle) => draw_rectangle(buffer, width, height, *rectangle),
            RenderCommand::Circle(circle) => draw_circle(buffer, width, height, *circle),
            RenderCommand::Image(image) => draw_image(buffer, width, height, image),
            RenderCommand::Text(text) => draw_text(buffer, width, height, text),
        }
    }
    for cursor in &frame.cursor {
        draw_rectangle(buffer, width, height, *cursor);
    }
}

fn draw_rectangle(buffer: &mut [u32], width: u32, height: u32, rectangle: RenderRect) {
    let (x0, y0, x1, y1) = clipped_bounds(
        rectangle.x,
        rectangle.y,
        rectangle.width,
        rectangle.height,
        width,
        height,
    );
    let color = rectangle.color.to_argb_u32();
    let stride = width as usize;

    for y in y0..y1 {
        buffer[y as usize * stride + x0 as usize..y as usize * stride + x1 as usize].fill(color);
    }
}

fn draw_rounded_rectangle(
    buffer: &mut [u32],
    width: u32,
    height: u32,
    rectangle: RenderRoundedRect,
) {
    let (x0, y0, x1, y1) = clipped_bounds(
        rectangle.x,
        rectangle.y,
        rectangle.width,
        rectangle.height,
        width,
        height,
    );
    let color = rectangle.color.to_argb_u32();
    let stride = width as usize;

    for y in y0 as i32..y1 as i32 {
        for x in x0 as i32..x1 as i32 {
            let coverage = rounded_rect_coverage(x, y, rectangle);
            if coverage > 0.0 {
                let index = y as usize * stride + x as usize;
                buffer[index] = blend_argb(buffer[index], color, coverage);
            }
        }
    }
}

fn draw_circle(buffer: &mut [u32], width: u32, height: u32, circle: RenderCircle) {
    let (x0, y0, x1, y1) = clipped_bounds(
        circle.x,
        circle.y,
        circle.diameter,
        circle.diameter,
        width,
        height,
    );
    let radius = circle.diameter as f32 / 2.0;
    let center_x = circle.x as f32 + radius;
    let center_y = circle.y as f32 + radius;
    let color = circle.color.to_argb_u32();
    let stride = width as usize;

    for y in y0 as i32..y1 as i32 {
        for x in x0 as i32..x1 as i32 {
            let dx = x as f32 + 0.5 - center_x;
            let dy = y as f32 + 0.5 - center_y;
            let coverage = (0.5 - ((dx * dx + dy * dy).sqrt() - radius)).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let index = y as usize * stride + x as usize;
                buffer[index] = blend_argb(buffer[index], color, coverage);
            }
        }
    }
}

fn draw_image(buffer: &mut [u32], width: u32, height: u32, image: &RenderImage) {
    if image.width == 0 || image.height == 0 || image.draw_width == 0 || image.draw_height == 0 {
        return;
    }

    let (x0, y0, x1, y1) = clipped_bounds(
        image.x,
        image.y,
        image.draw_width.min(i32::MAX as u32) as i32,
        image.draw_height.min(i32::MAX as u32) as i32,
        width,
        height,
    );
    let stride = width as usize;
    let source_stride = image.width as usize;

    for y in y0 as i32..y1 as i32 {
        let local_y = (y - image.y).max(0) as u32;
        let source_y = ((u64::from(local_y) * u64::from(image.height))
            / u64::from(image.draw_height))
        .min(u64::from(image.height - 1)) as usize;

        for x in x0 as i32..x1 as i32 {
            let coverage = image
                .clip
                .map_or(1.0, |clip| rounded_rect_coverage(x, y, clip));
            if coverage <= 0.0 {
                continue;
            }
            let local_x = (x - image.x).max(0) as u32;
            let source_x = ((u64::from(local_x) * u64::from(image.width))
                / u64::from(image.draw_width))
            .min(u64::from(image.width - 1)) as usize;
            if let Some(pixel) = image.pixels_argb.get(source_y * source_stride + source_x) {
                let index = y as usize * stride + x as usize;
                buffer[index] = blend_argb(buffer[index], *pixel, coverage);
            }
        }
    }
}

fn rounded_rect_coverage(x: i32, y: i32, rectangle: RenderRoundedRect) -> f32 {
    let top_radius = rectangle
        .radius
        .max(0)
        .min(rectangle.width / 2)
        .min(rectangle.height / 2);
    let bottom_radius = rectangle
        .bottom_radius
        .max(0)
        .min(rectangle.width / 2)
        .min(rectangle.height / 2);
    let pixel_x = x as f32 + 0.5;
    let pixel_y = y as f32 + 0.5;
    let left = rectangle.x as f32;
    let top = rectangle.y as f32;
    let right = (rectangle.x + rectangle.width) as f32;
    let bottom = (rectangle.y + rectangle.height) as f32;

    let corner = if top_radius > 0 && pixel_y < top + top_radius as f32 {
        if pixel_x < left + top_radius as f32 {
            Some((
                left + top_radius as f32,
                top + top_radius as f32,
                top_radius,
            ))
        } else if pixel_x > right - top_radius as f32 {
            Some((
                right - top_radius as f32,
                top + top_radius as f32,
                top_radius,
            ))
        } else {
            None
        }
    } else if bottom_radius > 0 && pixel_y > bottom - bottom_radius as f32 {
        if pixel_x < left + bottom_radius as f32 {
            Some((
                left + bottom_radius as f32,
                bottom - bottom_radius as f32,
                bottom_radius,
            ))
        } else if pixel_x > right - bottom_radius as f32 {
            Some((
                right - bottom_radius as f32,
                bottom - bottom_radius as f32,
                bottom_radius,
            ))
        } else {
            None
        }
    } else {
        None
    };

    let Some((center_x, center_y, radius)) = corner else {
        return 1.0;
    };
    let dx = pixel_x - center_x;
    let dy = pixel_y - center_y;
    (0.5 - ((dx * dx + dy * dy).sqrt() - radius as f32)).clamp(0.0, 1.0)
}

fn blend_argb(destination: u32, source: u32, coverage: f32) -> u32 {
    let source_alpha = ((source >> 24) & 0xff) as f32 / 255.0 * coverage;
    if source_alpha <= 0.0 {
        return destination;
    }
    if source_alpha >= 1.0 {
        return source | 0xff00_0000;
    }

    let inverse = 1.0 - source_alpha;
    let blend = |shift: u32| {
        ((((source >> shift) & 0xff_u32) as f32 * source_alpha
            + ((destination >> shift) & 0xff_u32) as f32 * inverse)
            .round() as u32)
            << shift
    };
    0xff00_0000_u32 | blend(16) | blend(8) | blend(0)
}

fn draw_text(buffer: &mut [u32], width: u32, height: u32, text: &RenderText) {
    let mut cursor_x = text.x;
    for character in text.text.chars().take(80) {
        draw_glyph(
            buffer, width, height, cursor_x, text.y, character, text.color,
        );
        cursor_x += 12;
    }
}

fn draw_glyph(
    buffer: &mut [u32],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    character: char,
    color: u32,
) {
    let stride = width as usize;
    for (row, bits) in glyph_rows(character).iter().enumerate() {
        for column in 0..5 {
            if bits & (1 << (4 - column)) == 0 {
                continue;
            }
            for dy in 0..2 {
                for dx in 0..2 {
                    let pixel_x = x + column * 2 + dx;
                    let pixel_y = y + row as i32 * 2 + dy;
                    if pixel_x >= 0
                        && pixel_y >= 0
                        && pixel_x < width as i32
                        && pixel_y < height as i32
                    {
                        buffer[pixel_y as usize * stride + pixel_x as usize] = color;
                    }
                }
            }
        }
    }
}

fn glyph_rows(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [15, 16, 16, 16, 16, 16, 15],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [15, 16, 16, 19, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        ':' => [0, 4, 4, 0, 4, 4, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '@' => [14, 17, 23, 21, 23, 16, 14],
        '~' => [0, 0, 9, 22, 0, 0, 0],
        ' ' => [0; 7],
        _ => [31, 17, 2, 4, 4, 0, 4],
    }
}

fn clipped_bounds(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    framebuffer_width: u32,
    framebuffer_height: u32,
) -> (u32, u32, u32, u32) {
    (
        x.max(0).min(framebuffer_width as i32) as u32,
        y.max(0).min(framebuffer_height as i32) as u32,
        x.saturating_add(width).max(0).min(framebuffer_width as i32) as u32,
        y.saturating_add(height)
            .max(0)
            .min(framebuffer_height as i32) as u32,
    )
}
