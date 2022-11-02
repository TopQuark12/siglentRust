#![feature(core_intrinsics)]
#![feature(linked_list_cursors)]


use log::*;
use plotters::prelude::*;
mod scope;
use scope::scope::*;

// use fugit::*;
// use std::{thread, time};
// fn sleep (millis : MillisDurationU64) {
//     thread::sleep(time::Duration::from_millis(millis.to_millis()));
// }

fn main() -> Result<(), Box<dyn std::error::Error>> {

    env_logger::init();
    
    let mut scope = Sds::new("192.168.1.91:5025", 12).unwrap();
    info!("{:?}", scope.query("*IDN?\n").unwrap());

    let (samples, _points, wave_info) = scope.get_samples("C1").unwrap();

    let root_drawing_area = BitMapBackend::new("images/ch1.png", (1920, 1080))
        .into_drawing_area();

    root_drawing_area.fill(&WHITE).unwrap();

    let mut chart = ChartBuilder::on(&root_drawing_area)
        .build_cartesian_2d(wave_info.t_min..wave_info.t_max, wave_info.v_min..wave_info.v_max)
        .unwrap();

    chart.draw_series(LineSeries::new(samples, &RED))?;

    root_drawing_area.present()?;

    Ok(())

}
