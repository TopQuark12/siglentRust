#![feature(core_intrinsics)]
#![feature(linked_list_cursors)]


use std::{iter::{zip, Sum}, intrinsics::{size_of, log10f32}};
use log::*;
use nom::number::complete::float;
use plotters::prelude::*;
mod scope;
use scope::scope::*;
use realfft::{RealFftPlanner, num_complex::Complex};
use rayon::prelude::*;

use fugit::*;
use std::{thread, time};
fn sleep (millis : MillisDurationU64) {
    thread::sleep(time::Duration::from_millis(millis.to_millis()));
}

const AVG: usize = 1;
const SMOOTH: usize = 50;

fn main() -> Result<(), Box<dyn std::error::Error>> {

    env_logger::init();
    
    let mut scope = Sds::new("192.168.1.91:5025", 12).unwrap();
    info!("{:?}", scope.query("*IDN?\n").unwrap());
    let _ = scope.query(":ACQuire:POINts?\n").unwrap();
    let sample_points = float::<_, ()>(scope.query(":ACQuire:POINts?\n")?).unwrap().1 as usize;
    
    scope.write(":TRIGger:MODE SINGle\n").unwrap();
    // let mut ratios = vec![];
    let mut ratio_sum = vec![Complex{ re: 0.0f32, im: 0.0f32 }; sample_points / 2 + 1];
    let mut mag_sum = vec![0.0_f32; sample_points / 2 + 1];
    let mut pha_sum = vec![0.0_f32; sample_points / 2 + 1];
    let mut freq_step = 0.0_f32;
    for _ in 0..AVG {

        while !scope.query("TRIG:STAT?\n").unwrap().contains(&"Stop") {
            sleep(10_u64.millis());
        }

        let ((mut v_samples_c1, t_samples_c1), points, wave_info) = scope.get_samples("C1").unwrap();
        let ((mut v_samples_c3, t_samples_c3), points, wave_info) = scope.get_samples("C3").unwrap();
        info!("{}", v_samples_c1.len());
        scope.write(":TRIGger:MODE SINGle\n").unwrap();
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(points);
        let mut spectrum_c1 = fft.make_output_vec();
        fft.process(&mut v_samples_c1, &mut spectrum_c1).unwrap();
        let mut spectrum_c3 = fft.make_output_vec();
        fft.process(&mut v_samples_c3, &mut spectrum_c3).unwrap();
        freq_step =  (1.0 / ((wave_info.t_max - wave_info.t_min) / (points as f32))) / (points as f32);
        let mut ratio: Vec<Complex<f32>> = Vec::with_capacity(spectrum_c1.capacity());
        ratio.extend::<Vec<Complex<f32>>>((0..spectrum_c1.len()).into_par_iter().map(|x| spectrum_c3[x] / spectrum_c1[x]).collect());
        // ratio_sum.extend::<Vec<Complex<f32>>>((0..spectrum_c1.len()).into_par_iter().map(|x| spectrum_c3[x] / spectrum_c1[x] + ratio_sum[x]).collect());
        // ratio_sum = (0..spectrum_c1.len()).into_par_iter().map(|x| spectrum_c3[x] / spectrum_c1[x] + ratio_sum[x]).collect();

        let mut mag: Vec<f32> = Vec::with_capacity(sample_points / 2 * size_of::<f32>());
        let mut pha: Vec<f32> = Vec::with_capacity(sample_points / 2 * size_of::<f32>());
        // let mut freq: Vec<f32> = Vec::with_capacity(sample_points / 2 * size_of::<f32>());

        mag.extend::<Vec<f32>>((0..sample_points / 2).into_par_iter().map(|x| 10.0 * ratio[x].norm().log10()).collect());
        pha.extend::<Vec<f32>>((0..sample_points / 2).into_par_iter().map(|x| ratio[x].arg().to_degrees()).collect());

        let mut mag_smooth: Vec<f32> = Vec::with_capacity(sample_points / 2 * size_of::<f32>());
        let mut pha_smooth: Vec<f32> = Vec::with_capacity(sample_points / 2 * size_of::<f32>());

        mag_smooth.extend::<Vec<f32>>((0 + SMOOTH / 2..sample_points / 2 - SMOOTH / 2).into_par_iter().map(|x| mag[x - SMOOTH / 2 .. x + SMOOTH / 2].iter().sum::<f32>() / SMOOTH as f32).collect());
        pha_smooth.extend::<Vec<f32>>((0 + SMOOTH / 2..sample_points / 2 - SMOOTH / 2).into_par_iter().map(|x| pha[x - SMOOTH / 2 .. x + SMOOTH / 2].iter().sum::<f32>() / SMOOTH as f32).collect());

        mag_sum = (0..sample_points / 2 - SMOOTH).into_par_iter().map(|x| mag_sum[x] + mag_smooth[x]).collect();
        pha_sum = (0..sample_points / 2 - SMOOTH).into_par_iter().map(|x| pha_sum[x] + pha_smooth[x]).collect();

    }

    let mut mag: Vec<f32> = Vec::with_capacity(sample_points / 2 * size_of::<f32>());
    let mut pha: Vec<f32> = Vec::with_capacity(sample_points / 2 * size_of::<f32>());

    mag.extend::<Vec<f32>>((0..sample_points / 2 - SMOOTH).into_par_iter().map(|x| mag_sum[x] / AVG as f32).collect());
    pha.extend::<Vec<f32>>((0..sample_points / 2 - SMOOTH).into_par_iter().map(|x| pha_sum[x] / AVG as f32).collect());

    let mut freq = vec![];
    freq.extend::<Vec<f32>>((0 + SMOOTH / 2..sample_points / 2 - SMOOTH / 2).into_par_iter().map(|x| x as f32 * freq_step).collect());

   
    let root_drawing_area = BitMapBackend::new("images/mag.png", (1280, 720))
        .into_drawing_area();

    root_drawing_area.fill(&WHITE).unwrap();

    let mut chart = ChartBuilder::on(&root_drawing_area)
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d((freq.clone().into_iter().reduce(f32::min).unwrap()..freq.clone().into_iter().reduce(f32::max).unwrap()).log_scale(), 
        mag.clone().into_iter().reduce(f32::min).unwrap()..mag.clone().into_iter().reduce(f32::max).unwrap())
        .unwrap();
        
    chart.configure_mesh().draw()?;
    chart.configure_series_labels().draw()?;    
    chart.draw_series(LineSeries::new(zip(freq.clone(), mag), &RED))?;
    
    root_drawing_area.present()?;

    let root_drawing_area = BitMapBackend::new("images/pha.png", (1280, 720))
        .into_drawing_area();

    root_drawing_area.fill(&WHITE).unwrap();

    let mut chart = ChartBuilder::on(&root_drawing_area)
        .margin(5)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d((freq.clone().into_iter().reduce(f32::min).unwrap()..freq.clone().into_iter().reduce(f32::max).unwrap()).log_scale(), 
        -180.0_f32..180.0_f32)
        .unwrap();
        
    chart.configure_mesh().draw()?;
    chart.configure_series_labels().draw()?;    
    chart.draw_series(LineSeries::new(zip(freq.clone(), pha), &BLUE))?;
    
    root_drawing_area.present()?;

    Ok(())

}
