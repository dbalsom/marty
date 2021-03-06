#![allow(dead_code)]
// Video module
// This module takes an internal representation from the cga module and actually draws the screen
// It also defines representational details such as colors
use std::rc::Rc;
use std::cell::RefCell;

use crate::cga::{self, CGACard, CGAColor, CGAPalette, DisplayMode, CursorInfo};
use crate::bus::BusInterface;

extern crate rand; 
use rand::{
    distributions::{Distribution, Standard},
    Rng,
}; 

pub const ATTR_BLUE_FG: u8 = 0b0000_0001;
pub const ATTR_GREEN_FG: u8 = 0b0000_0010;
pub const ATTR_RED_FG: u8 = 0b0000_0100;
pub const ATTR_BRIGHT_FG: u8 = 0b0000_1000;
pub const ATTR_BLUE_BG: u8 = 0b0001_0000;
pub const ATTR_GREEN_BG: u8 = 0b0010_0000;
pub const ATTR_RED_BG: u8 = 0b0100_0000;
pub const ATTR_BRIGHT_BG: u8 = 0b1000_0000;

// Font is encoded as a bit pattern with a span of 256 bits per row
static CGA_FONT: &'static [u8; 2048] = include_bytes!("cga_font.bin");

const CGA_FIELD_OFFSET: u32 = 8192;

const FONT_SPAN: u32 = 32;
const FONT_W: u32 = 8;
const FONT_H: u32 = 8;

const HIRES_GFX_W: u32 = 640;
const HIRES_GFX_H: u32 = 200;
const GFX_W: u32 = 320;
const GFX_H: u32 = 200;

const FRAME_W: u32 = 640;
const FRAME_H: u32 = 400;



// Random color generator
impl Distribution<CGAColor> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> CGAColor {
        // match rng.gen_range(0, 3) { // rand 0.5, 0.6, 0.7
        match rng.gen_range(0..=15) { // rand 0.8
            0 => CGAColor::Black,
            1 => CGAColor::Blue,       
            2 => CGAColor::Green,        
            3 => CGAColor::Cyan,       
            4 => CGAColor::Red,        
            5 => CGAColor::Magenta,      
            6 => CGAColor::Brown,      
            7 => CGAColor::White,       
            8 => CGAColor::BlackBright,  
            9 => CGAColor::BlueBright, 
            10 => CGAColor::GreenBright,  
            11 => CGAColor::CyanBright, 
            12 => CGAColor::RedBright,  
            13 => CGAColor::MagentaBright,
            14 => CGAColor::Yellow,
            _ => CGAColor::WhiteBright  
        }
    }
}

// Return a RGBA slice given a CGA color Enum
pub fn color_enum_to_rgba(color: &CGAColor) -> &'static [u8; 4] {
    
    match color {
        CGAColor::Black         => &[0x00u8, 0x00u8, 0x00u8, 0xFFu8],
        CGAColor::Blue          => &[0x00u8, 0x00u8, 0xAAu8, 0xFFu8],
        CGAColor::Green         => &[0x00u8, 0xAAu8, 0x00u8, 0xFFu8],
        CGAColor::Cyan          => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8],
        CGAColor::Red           => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8],
        CGAColor::Magenta       => &[0xAAu8, 0x00u8, 0xAAu8, 0xFFu8],
        CGAColor::Brown         => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8],
        CGAColor::White         => &[0xAAu8, 0xAAu8, 0xAAu8, 0xFFu8],
        CGAColor::BlackBright   => &[0x55u8, 0x55u8, 0x55u8, 0xFFu8],
        CGAColor::BlueBright    => &[0x55u8, 0x55u8, 0xFFu8, 0xFFu8],
        CGAColor::GreenBright   => &[0x55u8, 0xFFu8, 0x55u8, 0xFFu8],
        CGAColor::CyanBright    => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8],
        CGAColor::RedBright     => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8],
        CGAColor::MagentaBright => &[0xFFu8, 0x55u8, 0xFFu8, 0xFFu8],
        CGAColor::Yellow        => &[0xFFu8, 0xFFu8, 0x55u8, 0xFFu8],
        CGAColor::WhiteBright   => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8],
    }
}

pub fn get_cga_composite_color( bits: u8, palette: &CGAPalette ) -> &'static [u8; 4] {

    match (bits, palette) {

        (0b0000, CGAPalette::Monochrome(_)) => &[0x00, 0x00, 0x00, 0xFF], // Black
        (0b0001, CGAPalette::Monochrome(_)) => &[0x00, 0x68, 0x0C, 0xFF], // Forest Green
        (0b0010, CGAPalette::Monochrome(_)) => &[0x21, 0x2B, 0xBD, 0xFF], // Dark Blue
        (0b0011, CGAPalette::Monochrome(_)) => &[0x0D, 0x9E, 0xD5, 0xFF], // Cyan
        (0b0100, CGAPalette::Monochrome(_)) => &[0x85, 0x09, 0x6C, 0xFF], // Maroon
        (0b0101, CGAPalette::Monochrome(_)) => &[0x75, 0x73, 0x76, 0xFF], // Grey
        (0b0110, CGAPalette::Monochrome(_)) => &[0xAF, 0x36, 0xFF, 0xFF], // Magenta
        (0b0111, CGAPalette::Monochrome(_)) => &[0x9B, 0xA9, 0xFF, 0xFF], // Lilac
        (0b1000, CGAPalette::Monochrome(_)) => &[0x51, 0x47, 0x00, 0xFF], // Brown
        (0b1001, CGAPalette::Monochrome(_)) => &[0x42, 0xBD, 0x00, 0xFF], // Bright Green
        (0b1010, CGAPalette::Monochrome(_)) => &[0x51, 0x53, 0x51, 0xFF], // Darker Grey  0x70 0x74 0x70 actual values but this looks better in KQI
        (0b1011, CGAPalette::Monochrome(_)) => &[0x5D, 0xF4, 0x7A, 0xFF], // Lime Green
        (0b1100, CGAPalette::Monochrome(_)) => &[0xE5, 0x54, 0x1D, 0xFF], // Red-Orange
        (0b1101, CGAPalette::Monochrome(_)) => &[0xD7, 0xCB, 0x19, 0xFF], // Yellow
        (0b1110, CGAPalette::Monochrome(_)) => &[0xFF, 0x81, 0xF2, 0xFF], // Pink
        (0b1111, CGAPalette::Monochrome(_)) => &[0xFD, 0xFF, 0xFC, 0xFF], // White

        (0b0000, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x00, 0x00, 0xFF], // Black
        (0b0001, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x9A, 0xFF, 0xFF], // Blue #1
        (0b0010, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x42, 0xFF, 0xFF], // Dark Blue
        (0b0011, CGAPalette::MagentaCyanWhite(_)) => &[0x00, 0x90, 0xFF, 0xFF], // Blue #2
        (0b0100, CGAPalette::MagentaCyanWhite(_)) => &[0xAA, 0x4C, 0x00, 0xFF], // Brown
        (0b0101, CGAPalette::MagentaCyanWhite(_)) => &[0x84, 0xFA, 0xD2, 0xFF], // Lime Green
        (0b0110, CGAPalette::MagentaCyanWhite(_)) => &[0xB9, 0xA2, 0xAD, 0xFF], // Gray
        (0b0111, CGAPalette::MagentaCyanWhite(_)) => &[0x96, 0xF0, 0xFF, 0xFF], // Pale Blue
        (0b1000, CGAPalette::MagentaCyanWhite(_)) => &[0xCD, 0x1F, 0x00, 0xFF], // Dark red
        (0b1001, CGAPalette::MagentaCyanWhite(_)) => &[0xA7, 0xCD, 0xFF, 0xFF], // Lilac #1
        (0b1010, CGAPalette::MagentaCyanWhite(_)) => &[0xDC, 0x75, 0xFF, 0xFF], // Magenta
        (0b1011, CGAPalette::MagentaCyanWhite(_)) => &[0xB9, 0xC3, 0xFF, 0xFF], // Lilac #2
        (0b1100, CGAPalette::MagentaCyanWhite(_)) => &[0xFF, 0x5C, 0x00, 0xFF], // Orange-Red
        (0b1101, CGAPalette::MagentaCyanWhite(_)) => &[0xED, 0xFF, 0xCC, 0xFF], // Pale yellow
        (0b1110, CGAPalette::MagentaCyanWhite(_)) => &[0xFF, 0xB2, 0xA6, 0xFF], // Peach
        (0b1111, CGAPalette::MagentaCyanWhite(_)) => &[0xFF, 0xFF, 0xFF, 0xFF], // White
        _ => &[0x00, 0x00, 0x00, 0xFF], // Default black
    }
}

pub fn get_cga_gfx_color(bits: u8, palette: &CGAPalette, intensity: bool) -> &'static [u8; 4] {
    match (bits, palette, intensity) {
        // Monochrome
        (0b00, CGAPalette::Monochrome(_), false) => &[0x00u8, 0x00u8, 0x00u8, 0x00u8], // Black
        (0b01, CGAPalette::Monochrome(fg), false) => color_enum_to_rgba(fg), // Foreground color
        // Palette 0 - Low Intensity
        (0b00, CGAPalette::RedGreenYellow(bg), false) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedGreenYellow(_), false) => &[0x00u8, 0xAAu8, 0x00u8, 0xFFu8], // Green
        (0b10, CGAPalette::RedGreenYellow(_), false) => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8], // Red
        (0b11, CGAPalette::RedGreenYellow(_), false) => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8], // Brown
        // Palette 0 - High Intensity
        (0b00, CGAPalette::RedGreenYellow(bg), true) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedGreenYellow(_), true) => &[0x55u8, 0xFFu8, 0x55u8, 0xFFu8], // GreenBright
        (0b10, CGAPalette::RedGreenYellow(_), true) => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8], // RedBright
        (0b11, CGAPalette::RedGreenYellow(_), true) => &[0xFFu8, 0xFFu8, 0x55u8, 0xFFu8], // Yellow
        // Palette 1 - Low Intensity
        (0b00, CGAPalette::MagentaCyanWhite(bg), false) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::MagentaCyanWhite(_), false) => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8], // Cyan
        (0b10, CGAPalette::MagentaCyanWhite(_), false) => &[0xAAu8, 0x00u8, 0xAAu8, 0xFFu8], // Magenta
        (0b11, CGAPalette::MagentaCyanWhite(_), false) => &[0xAAu8, 0xAAu8, 0xAAu8, 0xFFu8], // Gray
        // Palette 1 - High Intensity
        (0b00, CGAPalette::MagentaCyanWhite(bg), true) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::MagentaCyanWhite(_), true) => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8], // CyanBright
        (0b10, CGAPalette::MagentaCyanWhite(_), true) => &[0xFFu8, 0x55u8, 0xFFu8, 0xFFu8], // MagentaBright
        (0b11, CGAPalette::MagentaCyanWhite(_), true) => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8], // WhiteBright
        // Palette 2 - Low Intensity
        (0b00, CGAPalette::RedCyanWhite(bg), false) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedCyanWhite(_), false) => &[0x00u8, 0xAAu8, 0xAAu8, 0xFFu8], // Cyan
        (0b10, CGAPalette::RedCyanWhite(_), false) => &[0xAAu8, 0x00u8, 0x00u8, 0xFFu8], // Red
        (0b11, CGAPalette::RedCyanWhite(_), false) => &[0xAAu8, 0x55u8, 0x00u8, 0xFFu8], // Gray
        // Palette 2 - High Intensity
        (0b00, CGAPalette::RedCyanWhite(bg), true) => color_enum_to_rgba(bg), // Background color
        (0b01, CGAPalette::RedCyanWhite(_), true) => &[0x55u8, 0xFFu8, 0xFFu8, 0xFFu8], // CyanBright
        (0b10, CGAPalette::RedCyanWhite(_), true) => &[0xFFu8, 0x55u8, 0x55u8, 0xFFu8], // RedBright
        (0b11, CGAPalette::RedCyanWhite(_), true) => &[0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8], // WhiteBright
        _=> &[0x00u8, 0x00u8, 0x00u8, 0xFFu8] // Default Black
    }
}


pub struct Video {
    mode: DisplayMode,
    cols: u32,
    rows: u32
}

impl Video {
    pub fn new() -> Self {
        Self {
            mode: DisplayMode::Mode3TextCo80,
            cols: 80,
            rows: 25
        }
    }

    pub fn draw(&self, frame: &mut [u8], cga: Rc<RefCell<CGACard>>, bus: &BusInterface, composite: bool) {

        let video_mem = bus.get_slice_at(cga::CGA_MEM_ADDRESS, cga::CGA_MEM_SIZE);

        let cga_card = cga.borrow();
        let mode_40_cols = cga_card.is_40_columns();


        match cga_card.get_display_mode() {
            DisplayMode::Disabled => {
                // Blank screen here?
                return
            }
            DisplayMode::Mode0TextBw40 | DisplayMode::Mode1TextCo40 | DisplayMode::Mode2TextBw80 | DisplayMode::Mode3TextCo80 => {
                let cursor = cga_card.get_cursor_info();
                let char_height = cga_card.get_character_height();
    
                self.draw_text_mode(cursor, frame, FRAME_W, FRAME_H, video_mem, char_height, mode_40_cols );
            }
            DisplayMode::Mode4LowResGraphics | DisplayMode::Mode5LowResAltPalette => {
                let (palette, intensity) = cga_card.get_palette();
                if !composite {
                    draw_gfx_mode2x(frame, FRAME_W, FRAME_H, video_mem, palette, intensity);
                }
                else {
                    draw_gfx_mode2x_composite(frame, FRAME_W, FRAME_H, video_mem, palette, intensity);
                }
            }
            DisplayMode::Mode6HiResGraphics => {
                let (palette, intensity) = cga_card.get_palette();
                if !composite {
                    draw_gfx_mode_highres2x(frame, FRAME_W, FRAME_H, video_mem, palette);
                }
                else {
                    draw_gfx_mode2x_composite(frame, FRAME_W, FRAME_H, video_mem, palette, intensity);
                }
                
            }
            DisplayMode::Mode7LowResComposite => {
                let (palette, intensity) = cga_card.get_palette();
                if !composite {
                    draw_gfx_mode_highres2x(frame, FRAME_W, FRAME_H, video_mem, palette);
                }
                else {
                    draw_gfx_mode2x_composite(frame, FRAME_W, FRAME_H, video_mem, palette, intensity);
                }                
            }
            _ => {
                // blank screen here?
            }
        }
    }

    pub fn draw_text_mode(
        &self, cursor: CursorInfo, 
        frame: &mut [u8], 
        frame_w: u32, 
        frame_h: u32, 
        mem: &[u8], 
        char_height: u8, 
        lowres: bool) {

        let mem_span = match lowres {
            true => 40,
            false => 80
        };

        if char_height < 2 {
            return
        }

        let char_height = char_height as u32;

        let max_y = frame_h / char_height as u32 - 1;

        for (i, char) in mem.chunks_exact(2).enumerate() {
            let x = (i % mem_span as usize) as u32;
            let y = (i / mem_span as usize) as u32;
            
            //println!("x: {} y: {}", x, y);
            //pixel.copy_from_slice(&rgba);
            if y > max_y {
                break;
            }

            let (fg_color, bg_color) = get_colors_from_attr_byte(char[1]);

            match lowres {
                true => draw_glyph4x(char[0], fg_color, bg_color, frame, frame_w, frame_h, char_height, x * 8, y * char_height),
                false => draw_glyph2x(char[0], fg_color, bg_color, frame, frame_w, frame_h, char_height, x * 8, y * char_height)
            }

        }

        match lowres {
            true => draw_cursor4x(cursor, frame, FRAME_W, FRAME_H, mem ),
            false => draw_cursor2x(cursor, frame, FRAME_W, FRAME_H, mem )
        }
    }


}

pub fn draw_gfx_mode2x(frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8], pal: CGAPalette, intensity: bool) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)
    
    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(GFX_H / 2) {

            // CGA gfx mode = 2 bits (4 pixels per byte). Double line count to skip every other line
            let src_y_idx = draw_y * (GFX_W / 4) + field_src_offset; 
            let dst_span = (FRAME_W) * 4;
            let dst1_y_idx = draw_y * (dst_span * 4) + field_dst_offset;  // RBGA = 4 bytes x 2x pixels
            let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

            // Draw 4 pixels at a time
            for draw_x in 0..(GFX_W / 4) {

                let dst1_x_idx = (draw_x * 4) * 4 * 2;
                let dst2_x_idx = dst1_x_idx + 4;

                let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                // Four pixels in a byte
                for pix_n in 0..4 {
                    // Mask the pixel bits, right-to-left
                    let shift_ct = 8 - (pix_n * 2) - 2;
                    let pix_bits = cga_byte >> shift_ct & 0x03;
                    // Get the RGBA for this pixel
                    let color = get_cga_gfx_color(pix_bits, &pal, intensity);
                    // Draw first row of pixel 2x
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 3] = color[3];

                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 3] = color[3];

                    // Draw 2nd row of pixel 2x
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 8)) as usize + 3] = color[3];      

                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 8)) as usize + 3] = color[3];                                    
                }
            }
        }
        field_src_offset += CGA_FIELD_OFFSET;
        field_dst_offset += (FRAME_W) * 4 * 2;
    }
}

pub fn draw_gfx_mode_highres2x(frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8], pal: CGAPalette) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)
    
    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(HIRES_GFX_H / 2) {

            // CGA hi-res gfx mode = 1 bpp (8 pixels per byte).

            let src_y_idx = draw_y * (HIRES_GFX_W / 8) + field_src_offset; 

            let dst_span = FRAME_W * 4;
            let dst1_y_idx = draw_y * (dst_span * 4) + field_dst_offset;  // RBGA = 4 bytes x 2x pixels
            let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

            // Draw 8 pixels at a time
            for draw_x in 0..(HIRES_GFX_W / 8) {

                let dst1_x_idx = (draw_x * 8) * 4;

                let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                // Eight pixels in a byte
                for pix_n in 0..8 {
                    // Mask the pixel bits, right-to-left
                    let shift_ct = 8 - pix_n - 1;
                    let pix_bit = cga_byte >> shift_ct & 0x01;
                    // Get the RGBA for this pixel
                    let color = get_cga_gfx_color(pix_bit, &pal, false);
                    // Draw first row of pixel
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 3] = color[3];

                    // Draw 2nd row of pixel
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 4)) as usize + 3] = color[3];      
                }
            }
        }
        field_src_offset += CGA_FIELD_OFFSET;
        field_dst_offset += (FRAME_W) * 4 * 2;
    }
}


pub fn draw_gfx_mode2x_composite(frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8], pal: CGAPalette, intensity: bool) {
    // First half of graphics memory contains all EVEN rows (0, 2, 4, 6, 8)
    
    let mut field_src_offset = 0;
    let mut field_dst_offset = 0;
    for _field in 0..2 {
        for draw_y in 0..(GFX_H / 2) {

            // CGA gfx mode = 2 bits (4 pixels per byte). Double line count to skip every other line
            let src_y_idx = draw_y * (GFX_W / 4) + field_src_offset; 
            let dst_span = (FRAME_W) * 4;
            let dst1_y_idx = draw_y * (dst_span * 4) + field_dst_offset;  // RBGA = 4 bytes x 2x pixels
            let dst2_y_idx = draw_y * (dst_span * 4) + dst_span + field_dst_offset;  // One scanline down

            // Draw 4 pixels at a time
            for draw_x in 0..(GFX_W / 4) {

                let dst1_x_idx = (draw_x * 4) * 4 * 2;
                let dst2_x_idx = dst1_x_idx + 4;
                let dst3_x_idx = dst1_x_idx + 8;
                let dst4_x_idx = dst1_x_idx + 12;

                let cga_byte: u8 = mem[(src_y_idx + draw_x) as usize];

                // Two composite 'pixels' in a byte
                for pix_n in 0..2 {
                    // Mask the pixel bits, right-to-left
                    let shift_ct = 8 - (pix_n * 4) - 4;
                    let pix_bits = cga_byte >> shift_ct & 0x0F;
                    // Get the RGBA for this pixel
                    let color = get_cga_composite_color(pix_bits, &pal);
                    // Draw first row of pixel 4x
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 3] = color[3];

                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 3] = color[3];

                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 3] = color[3];
                    
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst1_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 3] = color[3];                    

                    // Draw 2nd row of pixel 4x
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst1_x_idx + (pix_n * 16)) as usize + 3] = color[3];      

                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst2_x_idx + (pix_n * 16)) as usize + 3] = color[3];      

                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst3_x_idx + (pix_n * 16)) as usize + 3] = color[3];    

                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize]     = color[0];
                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 1] = color[1];
                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 2] = color[2];
                    frame[(dst2_y_idx + dst4_x_idx + (pix_n * 16)) as usize + 3] = color[3];    
                }
            }
        }
        field_src_offset += CGA_FIELD_OFFSET;
        field_dst_offset += (FRAME_W) * 4 * 2;
    }
}

pub fn get_colors_from_attr_byte(byte: u8) -> (CGAColor, CGAColor) {

    let fg_nibble = byte & 0x0F;
    let bg_nibble = (byte >> 4 ) & 0x0F;

    let bg_color = get_colors_from_attr_nibble(bg_nibble);
    let fg_color = get_colors_from_attr_nibble(fg_nibble);

    (fg_color, bg_color)
}

pub fn get_colors_from_attr_nibble(byte: u8) -> CGAColor {

    match byte {
        0b0000 => CGAColor::Black,
        0b0001 => CGAColor::Blue,
        0b0010 => CGAColor::Green,
        0b0100 => CGAColor::Red,
        0b0011 => CGAColor::Cyan,
        0b0101 => CGAColor::Magenta,
        0b0110 => CGAColor::Brown,
        0b0111 => CGAColor::White,
        0b1000 => CGAColor::BlackBright,
        0b1001 => CGAColor::BlueBright,
        0b1010 => CGAColor::GreenBright,
        0b1100 => CGAColor::RedBright,
        0b1011 => CGAColor::CyanBright,
        0b1101 => CGAColor::MagentaBright,
        0b1110 => CGAColor::Yellow,
        0b1111 => CGAColor::WhiteBright,
        _=> CGAColor::Black
    }
}

// Draw a CGA font glyph in 40 column mode at an arbitrary location
pub fn draw_glyph4x( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    char_height: u32,
    pos_x: u32, 
    pos_y: u32 ) -> () {

    // Do not draw glyph off screen
    if (pos_x + (FONT_W * 2) > frame_w) || (pos_y * 2 + (FONT_H * 2 ) > frame_h) {
        return
    }

    // Find the source position of the glyph
    let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 

    for draw_glyph_y in 0..char_height {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
        
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;

        let glyph_byte: u8 = CGA_FONT[glyph_offset as usize];

        for draw_glyph_x in 0..FONT_W {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(&fg_color)
            }
            else {
                color_enum_to_rgba(&bg_color)
            };

            let dst_offset = dst_row_offset + ((pos_x * 2) + (draw_glyph_x*2)) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];

            frame[(dst_offset + 4) as usize] = color[0];
            frame[(dst_offset + 4) as usize + 1] = color[1];
            frame[(dst_offset + 4) as usize + 2] = color[2];
            frame[(dst_offset + 4) as usize + 3] = color[3];


            let dst_offset2 = dst_row_offset2 + ((pos_x * 2) + (draw_glyph_x*2)) * 4;
            frame[dst_offset2 as usize] = color[0];
            frame[dst_offset2 as usize + 1] = color[1];
            frame[dst_offset2 as usize + 2] = color[2];
            frame[dst_offset2 as usize + 3] = color[3];   

            frame[(dst_offset2 + 4 ) as usize] = color[0];
            frame[(dst_offset2 + 4) as usize + 1] = color[1];
            frame[(dst_offset2 + 4) as usize + 2] = color[2];
            frame[(dst_offset2 + 4) as usize + 3] = color[3];    
        }
    }     
}

// Draw a CGA font glyph in 80 column mode at an arbitrary location
pub fn draw_glyph2x( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    char_height: u32,
    pos_x: u32, 
    pos_y: u32 ) -> () {

    // Do not draw glyph off screen
    if pos_x + FONT_W > frame_w {
        return
    }
    if pos_y * 2 + (FONT_H * 2 ) > frame_h {
        return
    }

    // Find the source position of the glyph
    let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 

    for draw_glyph_y in 0..char_height {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
        
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;

        let glyph_byte: u8 = CGA_FONT[glyph_offset as usize];

        for draw_glyph_x in 0..FONT_W {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(&fg_color)
            }
            else {
                color_enum_to_rgba(&bg_color)
            };

            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];

            let dst_offset2 = dst_row_offset2 + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset2 as usize] = color[0];
            frame[dst_offset2 as usize + 1] = color[1];
            frame[dst_offset2 as usize + 2] = color[2];
            frame[dst_offset2 as usize + 3] = color[3];            
        }
    }     
}

pub fn draw_cursor4x(cursor: CursorInfo, frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8] ) {
        
    // First off, is cursor even visible?
    if !cursor.visible {
        return
    }
    
    // Do not draw cursor off screen
    let pos_x = cursor.pos_x * 8;
    let pos_y = cursor.pos_y * 8;
    if (pos_x + (FONT_W * 2) > frame_w) || (pos_y * 2 + (FONT_H * 2 ) > frame_h) {
        return
    }

    // Cursor start register can be greater than end register, in this case no cursor is shown
    if cursor.line_start > cursor.line_end {
        return
    }

    let line_start = cursor.line_start as u32;
    let mut line_end = cursor.line_end as u32;

    // Clip cursor if at bottom of screen and cursor.line_end > FONT_H
    if pos_y * 2 + line_end * 2 >= frame_h {
        line_end -= frame_h - (pos_y * 2 + line_end * 2) + 1;
    }        

    // Is character attr in mem range?
    let attr_addr = (cursor.addr * 2 + 1) as usize;
    if attr_addr > mem.len() {
        return
    }
    let cursor_attr: u8 = mem[attr_addr];
    let (fg_color, _bg_color) = get_colors_from_attr_byte(cursor_attr);
    let color = color_enum_to_rgba(&fg_color);

    for draw_glyph_y in line_start..line_end {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
        
        for draw_glyph_x in 0..FONT_W {
        
            let dst_offset = dst_row_offset + ((pos_x * 2) + (draw_glyph_x*2)) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];

            frame[(dst_offset + 4) as usize] = color[0];
            frame[(dst_offset + 4) as usize + 1] = color[1];
            frame[(dst_offset + 4) as usize + 2] = color[2];
            frame[(dst_offset + 4) as usize + 3] = color[3];

            let dst_offset2 = dst_row_offset2 + ((pos_x * 2) + (draw_glyph_x*2)) * 4;
            frame[dst_offset2 as usize] = color[0];
            frame[dst_offset2 as usize + 1] = color[1];
            frame[dst_offset2 as usize + 2] = color[2];
            frame[dst_offset2 as usize + 3] = color[3];   

            frame[(dst_offset2 + 4 ) as usize] = color[0];
            frame[(dst_offset2 + 4) as usize + 1] = color[1];
            frame[(dst_offset2 + 4) as usize + 2] = color[2];
            frame[(dst_offset2 + 4) as usize + 3] = color[3];    
        }
    }    
}

/// Draw the cursor as a character cell into the specified framebuffer with 2x height
pub fn draw_cursor2x(cursor: CursorInfo, frame: &mut [u8], frame_w: u32, frame_h: u32, mem: &[u8] ) {
    
    // First off, is cursor even visible?
    if !cursor.visible {
        return
    }
    
    // Do not draw cursor off screen
    let pos_x = cursor.pos_x * 8;
    let pos_y = cursor.pos_y * 8;

    let max_pos_x = pos_x + FONT_W; 
    let max_pos_y = pos_y * 2 + (FONT_H * 2);  
    if max_pos_x > frame_w || max_pos_y > frame_h {
        return
    }

    // Cursor start register can be greater than end register, in this case no cursor is shown
    if cursor.line_start > cursor.line_end {
        return
    }

    let line_start = cursor.line_start as u32;
    let mut line_end = cursor.line_end as u32;

    // Clip cursor if at bottom of screen and cursor.line_end > FONT_H
    if pos_y * 2 + line_end * 2 >= frame_h {
        line_end -= frame_h - (pos_y * 2 + line_end * 2) + 1;
    }

    // Is character attr in mem range?
    let attr_addr = (cursor.addr * 2 + 1) as usize;
    if attr_addr > mem.len() {
        return
    }
    let cursor_attr: u8 = mem[attr_addr];
    let (fg_color, _bg_color) = get_colors_from_attr_byte(cursor_attr);
    let color = color_enum_to_rgba(&fg_color);

    for draw_glyph_y in line_start..=line_end {

        let dst_row_offset = frame_w * 4 * ((pos_y * 2) + (draw_glyph_y*2));
        let dst_row_offset2 = dst_row_offset + (frame_w * 4);
                                    
        for draw_glyph_x in 0..FONT_W {
        
            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];

            let dst_offset2 = dst_row_offset2 + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset2 as usize] = color[0];
            frame[dst_offset2 as usize + 1] = color[1];
            frame[dst_offset2 as usize + 2] = color[2];
            frame[dst_offset2 as usize + 3] = color[3];   

        }
    }                 
}    

// Draw a CGA font glyph at an arbitrary location
pub fn draw_glyph( 
    glyph: u8,
    fg_color: CGAColor,
    bg_color: CGAColor,
    frame: &mut [u8], 
    frame_w: u32, 
    frame_h: u32, 
    char_height: u32,
    pos_x: u32, 
    pos_y: u32 ) -> () {

    // Do not draw glyph off screen
    if pos_x + FONT_W > frame_w {
        return
    }
    if pos_y + FONT_H > frame_h {
        return
    }

    // Find the source position of the glyph
    let glyph_offset_src_x = glyph as u32 % FONT_SPAN;
    let glyph_offset_src_y = (glyph as u32 / FONT_SPAN) * (FONT_H * FONT_SPAN); 

    for draw_glyph_y in 0..FONT_H {

        let dst_row_offset = frame_w * 4 * (pos_y + draw_glyph_y);
        let glyph_offset = glyph_offset_src_y + (draw_glyph_y * FONT_SPAN) + glyph_offset_src_x;

        let glyph_byte: u8 = CGA_FONT[glyph_offset as usize];

        for draw_glyph_x in 0..FONT_W {
        
            let test_bit: u8 = 0x80u8 >> draw_glyph_x;

            let color = if test_bit & glyph_byte > 0 {
                color_enum_to_rgba(&fg_color)
            }
            else {
                color_enum_to_rgba(&bg_color)
            };

            let dst_offset = dst_row_offset + (pos_x + draw_glyph_x) * 4;
            frame[dst_offset as usize] = color[0];
            frame[dst_offset as usize + 1] = color[1];
            frame[dst_offset as usize + 2] = color[2];
            frame[dst_offset as usize + 3] = color[3];
        }
    }
}
