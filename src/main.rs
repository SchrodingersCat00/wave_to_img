use core::fmt::Debug;
use image::{Rgb, RgbImage};
use num::{clamp, Bounded, Float, ToPrimitive, Zero};
use std::fs::File;
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
        let max = *wave.data[sp..sp + samples_per_pixel].iter().max().unwrap();
        let min = *wave.data[sp..sp + samples_per_pixel].iter().min().unwrap();

        // println!("min: {:?}, max: {:?}", min, max);

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

    image
}

fn demo() {
    let mut inp_file = File::open(Path::new("data/20Hz_mono.wav")).expect("could not open file");
    let (header, data) = wav::read(&mut inp_file).expect("Coult not read wav file");
    assert!(data.is_sixteen());
    let image = draw_waveform(
        3000,
        1500,
        &Wave::<i16> {
            data: data.as_sixteen().unwrap(),
            channel_count: header.channel_count,
        },
        Rgb([0, 0, 0]),
        Rgb([255, 255, 255]),
    );
    image.save("data/out.png").expect("Error while saving the image");
}

fn main() {
    demo();
}
