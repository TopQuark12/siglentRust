use std::cmp::min;
use std::intrinsics::size_of;
use std::io::ErrorKind;
use std::net::*;
use std::str;
use std::io::prelude::*;
use std::io::{Result, Error};
use log::*;
use byteorder::*;
use nom::number::complete::float;
use rayon::prelude::*;

use fugit::*;
use std::{thread, time};
fn sleep (millis : MillisDurationU64) {
    thread::sleep(time::Duration::from_millis(millis.to_millis()));
}

const RX_BUF_SIZE: usize = 1024;

pub struct WaveInfo {
    pub t_max: f32,
    pub t_min: f32,
    pub v_max: f32,
    pub v_min: f32,
}

pub struct Sds {
    stream: TcpStream,
    rx_buf: [u8; RX_BUF_SIZE],
    rx_len: usize,
    bits: usize
}

impl Sds {

    pub fn new<A : ToSocketAddrs>(ip: A, bits: usize) -> Result<Sds> {
        Ok(Sds {
            stream: TcpStream::connect(ip)?,
            rx_buf: [0_u8 ; RX_BUF_SIZE],
            rx_len: 0,
            bits: bits
        }).and_then(|sds| {info!("Connected to the scope!"); Ok(sds)})
    }
    pub fn write(&mut self, command: &str) -> Result<()> {
        self.stream.write(command.as_bytes())?;
        Ok(())
    }

    pub fn read_raw(&mut self) -> Result<&[u8]>{
        self.stream.read(&mut self.rx_buf).and_then(|len| {self.rx_len = len; Ok(&self.rx_buf[0..len])})
    }

    pub fn read(&mut self) -> Result<&[u8]>{
        // self.stream.read(&mut self.rx_buf).and_then(|len| {self.rx_len = len; Ok(&self.rx_buf[0..len])})
        self.rx_len = 0;
        self.rx_buf.fill(0_u8);
        while {
            self.stream.read(&mut self.rx_buf[self.rx_len..]).and_then(|len| {self.rx_len += len; Ok(&self.rx_buf[0..len])}).unwrap();
            !self.rx_buf.contains(&10)
        } {}
        Ok(&self.rx_buf[0..self.rx_len])
    }

    pub fn query_raw(&mut self, command: &str) -> Result<&[u8]> {
        self.write(command)?;
        self.read()
    }

    pub fn query(&mut self, command: &str) -> Result<&str> {
        self.query_raw(command)?;
        info!("{:?}", str::from_utf8(&self.rx_buf[0..self.rx_len]));
        match str::from_utf8(&self.rx_buf[0..self.rx_len]) {
            Ok(msg) => Ok(msg),
            Err(e) => Err(Error::new(ErrorKind::InvalidData, e))
        } 
    }

    pub fn get_wave_parameter(&mut self) -> Result<(f32, f32, f32, f32)> {
        let raw_waveform_settings = self.query_raw(":WAVeform:PREamble?\n")?;
        let waveform_settings = raw_waveform_settings.split(|x| x == &b'#').nth(1).unwrap();
        info!("raw {:?}", waveform_settings);
        let volt_per_div = f32::from_le_bytes(waveform_settings[156+10..160+10].try_into().unwrap());
        let vert_offset = f32::from_le_bytes(waveform_settings[160+10..164+10].try_into().unwrap());
        let lsb_per_div = f32::from_le_bytes(waveform_settings[164+10..168+10].try_into().unwrap());
        let probe_atten = f32::from_le_bytes(waveform_settings[328+10..332+10].try_into().unwrap());
        let volt_per_lsb = volt_per_div / lsb_per_div * probe_atten;    
        let sample_interval = f32::from_le_bytes(waveform_settings[176+10..180+10].try_into().unwrap());
        Ok((volt_per_lsb, vert_offset, sample_interval, lsb_per_div * 8.0))
    }

    pub fn get_samples(&mut self, ch: &str) -> Result<((Vec<f32>, Vec<f32>), usize, WaveInfo)> {

        if self.bits > 8 {
            self.write(":WAVeform:WIDTh WORD\n")?;
        } else {
            self.write(":WAVeform:WIDTh BYTE\n")?;
        }

        self.write(&*format!(":WAVeform:SOURce {}\n", ch))?;
        let (volt_per_lsb, vert_offset, sample_interval, vert_bits) = self.get_wave_parameter()?;   
        info!("got wave param {} {} {} {}", volt_per_lsb, vert_offset, sample_interval, vert_bits);
        let _ = self.query(":ACQuire:POINts?\n").unwrap();
        let sample_points = float::<_, ()>(self.query(":ACQuire:POINts?\n")?).unwrap().1 as usize;
        let max_point_transfer = float::<_, ()>(self.query(":WAV:MAXPoint?\n")?).unwrap().1 as usize;
        let num_transfer_req = ((sample_points + max_point_transfer - 1) / max_point_transfer) as usize;
        let samples_to_receive = min(max_point_transfer, sample_points);
        let bytes_per_sample = (self.bits + 7) / 8;
        let bytes_to_receive = samples_to_receive * bytes_per_sample + 11 + 2;
        info!("samples {}", sample_points);
        info!("max tx {}", max_point_transfer);
        info!("tx needed {}", num_transfer_req);
        info!("bytes to receive {:?}", bytes_to_receive);
        let mut v_samples: Vec<f32> = Vec::with_capacity(sample_points * size_of::<f32>());
        let mut t_samples: Vec<f32> = Vec::with_capacity(sample_points * size_of::<f32>());
        let info = WaveInfo {
            t_max: sample_interval * sample_points as f32,
            t_min: 0.0,
            v_max:   volt_per_lsb * vert_bits / 2.0 + vert_offset,
            v_min: - volt_per_lsb * vert_bits / 2.0 + vert_offset,
        };
        for transfer_cnt in 0..num_transfer_req {
            let mut transfer_buf: Vec<u8> = Vec::with_capacity(bytes_to_receive + 1024);
            self.write(&*format!(":WAVeform:STARt {}\n", transfer_cnt * max_point_transfer))?;
            self.write(":WAVeform:DATA?\n")?;
            let mut bytes_received = 0;
            while bytes_received < bytes_to_receive {
                transfer_buf.extend_from_slice(self.read_raw().unwrap());
                bytes_received += self.rx_len;
            }
            info!("{:?} bytes received", bytes_received);
            let _: Vec<_>= transfer_buf.drain(0..11).collect();
            if self.bits > 8 {
                v_samples.extend::<Vec<f32>>(transfer_buf.par_chunks(2).map(|x|{LittleEndian::read_i16(x) as f32 * volt_per_lsb}).collect());
                t_samples.extend::<Vec<f32>>((samples_to_receive * transfer_cnt..samples_to_receive * transfer_cnt + samples_to_receive)
                                                .into_par_iter().map(|x| x as f32 * sample_interval).collect());
                            
            } else {
                v_samples.extend::<Vec<f32>>(transfer_buf.into_par_iter().map(|x| (x as i8) as f32 * volt_per_lsb).collect());
                t_samples.extend::<Vec<f32>>((samples_to_receive * transfer_cnt..samples_to_receive * transfer_cnt + samples_to_receive)
                                                .into_par_iter().map(|x| x as f32 * sample_interval).collect());
            }

        }

        info!("vmax {:?}", info.v_max);
        info!("vmin {:?}", info.v_min);
        info!("tmax {:?}", info.t_max);
        info!("tmin {:?}", info.t_min);

        v_samples.pop();
        t_samples.pop();

        Ok(((v_samples, t_samples), sample_points, info))

    }

}

impl Drop for Sds {
    fn drop(&mut self) {
        self.stream.shutdown(Shutdown::Both).unwrap();
        info!("Connection to scope closed");
    }
}
