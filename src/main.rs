use argparse::{ArgumentParser, Store};
use core::fmt::Debug;
use image::{Rgb, RgbImage};
use num::{clamp, Bounded, Float, ToPrimitive, Zero};
use std::fs::File;
use std::num::ParseIntError;
use std::path::Path;
use wav;

fn scale_to_range<T: Float>(value: T, in_begin: T, in_end: T, out_begin: T, out_end: T) -> T {
    out_begin + (value - in_begin) / (in_end - in_begin) * (out_end - out_begin)
}

fn clamp_scale<T: Float + Debug>(value: T, in_begin: T, in_end: T, out_begin: T, out_end: T) -> T {
    clamp(
        scale_to_range(value, in_begin, in_end, out_begin, out_end),
        out_begin,
        out_end,
    )
}

fn saturating_cast(value: usize) -> u32 {
    value.try_into().unwrap_or(u32::MAX)
}

fn upscale_image(image: &RgbImage, new_width: u32) -> RgbImage {
    let mut output_image = RgbImage::new(new_width, image.height());
    for row in 0..output_image.height() {
        for column in 0..output_image.width() {
            let pi = column * image.width() / output_image.width();
            output_image.put_pixel(column, row, *image.get_pixel(pi, row));
        }
    }

    output_image
}

struct Wave<'a, T> {
    data: &'a Vec<T>,
    channel_count: u16,
}

impl<'a, T> Wave<'a, T> {
    fn frame_count(&self) -> usize {
        assert!(self.data.len() % self.channel_count as usize == 0);
        self.data.len() / self.channel_count as usize
    }

    fn sample_count(&self) -> usize {
        self.data.len()
    }
}

type Color = Rgb<u8>;

// TODO: This will not work for SampleType's that are unsigned
fn draw_waveform<'a, SampleType: Ord + Zero + Into<f64> + Bounded + Copy + Debug>(
    width: usize,
    height: usize,
    wave: &'a Wave<SampleType>,
    fg_color: Color,
    bg_color: Color,
) -> RgbImage {
    let small_wave = wave.frame_count() < width;
    let mut image = if small_wave {
        RgbImage::new(saturating_cast(wave.frame_count()), saturating_cast(height))
    } else {
        RgbImage::new(saturating_cast(width), saturating_cast(height))
    };

    let samples_per_pixel = wave.sample_count() / image.width() as usize;

    for column in 0..image.width() as usize {
        let sp = column * samples_per_pixel;

        // TODO: maybe loop manually to make better use of cache
        let max = *wave.data[sp..sp + samples_per_pixel].iter().max().unwrap();
        let min = *wave.data[sp..sp + samples_per_pixel].iter().min().unwrap();

        let top_pixel = clamp_scale(
            max.into(),
            0.,
            SampleType::max_value().into(),
            image.height().to_f64().unwrap() / 2.,
            image.height().to_f64().unwrap(),
        )
        .round()
        .to_u32()
        .unwrap();

        let bottom_pixel = clamp_scale(
            min.into(),
            SampleType::min_value().into(),
            SampleType::zero().into(),
            0.,
            image.height().to_f64().unwrap() / 2.,
        )
        .round()
        .to_u32()
        .unwrap();

        for row in 0..bottom_pixel {
            image.put_pixel(saturating_cast(column), row, bg_color);
        }
        for row in bottom_pixel..top_pixel {
            image.put_pixel(saturating_cast(column), row, fg_color);
        }
        for row in top_pixel..image.height() {
            image.put_pixel(saturating_cast(column), row, bg_color);
        }
    }

    if small_wave {
        image = upscale_image(&image, width as u32);
    }

    image
}

fn generate_png(
    inp_file_path: &String,
    out_file_path: &String,
    height: usize,
    width: usize,
    fg_color: Color,
    bg_color: Color,
) {
    let mut inp_file = File::open(Path::new(inp_file_path)).expect("could not open file");
    let (header, data) = wav::read(&mut inp_file).expect("Coult not read wav file");
    assert!(data.is_sixteen());
    let image = draw_waveform(
        width,
        height,
        &Wave::<i16> {
            data: data.as_sixteen().unwrap(),
            // data: &vec![i16::MAX/2, i16::MAX, i16::MIN, i16::MIN/2],
            channel_count: header.channel_count,
        },
        fg_color,
        bg_color,
    );
    image
        .save(out_file_path)
        .expect("Error while saving the image");
}

fn parse_hex_color(hex_color: &str) -> Result<Color, ParseIntError> {
    let hex = &hex_color[1..]; // remove the "#" symbol
    let r = u8::from_str_radix(&hex[0..2], 16)?;
    let g = u8::from_str_radix(&hex[2..4], 16)?;
    let b = u8::from_str_radix(&hex[4..6], 16)?;
    Ok(Rgb([r, g, b]))
}

struct Options {
    width: usize,
    height: usize,
    fg_color: String,
    bg_color: String,
    input_file: String,
    output_file: String,
}

fn main() {
    let mut options = Options {
        width: 1000,
        height: 250,
        fg_color: "#000000".to_string(),
        bg_color: "#ffffff".to_string(),
        input_file: "".to_string(),
        output_file: "out.png".to_string(),
    };

    {
        // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();
        ap.set_description("Generate image thumbnails for audio files");
        ap.refer(&mut options.width).add_option(
            &["-w", "--width"],
            Store,
            "Output image width (in pixels)",
        );
        ap.refer(&mut options.height).add_option(
            &["-h", "--height"],
            Store,
            "Output image height (in pixels)",
        );
        ap.refer(&mut options.fg_color)
            .add_option(&["--fg-color"], Store, "Foreground color");
        ap.refer(&mut options.bg_color)
            .add_option(&["--bg-color"], Store, "Background color");
        ap.refer(&mut options.input_file)
            .add_option(&["-i", "--input"], Store, "Input file path");
        ap.refer(&mut options.output_file).add_option(
            &["-o", "--output"],
            Store,
            "Output file path",
        );
        ap.parse_args_or_exit();
    }

    generate_png(
        &options.input_file,
        &options.output_file,
        options.height,
        options.width,
        parse_hex_color(&options.fg_color).expect("Invalid fb color string"),
        parse_hex_color(&options.bg_color).expect("Invalid bg color string"),
    )
}
