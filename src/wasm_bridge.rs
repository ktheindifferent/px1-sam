use crate::psx::Psx;
use crate::error::Error;

pub trait PsxInterface {
    fn new() -> Self;
    fn load_bios(&mut self, data: &[u8]) -> Result<(), Error>;
    fn load_disc(&mut self, data: &[u8]) -> Result<(), Error>;
    fn run_frame(&mut self) -> Result<(), Error>;
    fn set_controller_state(&mut self, port: u8, state: u16);
    fn get_display_size(&self) -> (u32, u32);
    fn get_framebuffer(&mut self, buffer: &mut [u8]);
    fn get_audio_samples(&mut self, buffer: &mut [f32]);
    fn serialize_state(&self) -> Vec<u8>;
    fn deserialize_state(&mut self, data: &[u8]) -> Result<(), Error>;
}

impl PsxInterface for Psx {
    fn new() -> Self {
        let mut psx = Psx::default();
        psx.init();
        psx
    }

    fn load_bios(&mut self, data: &[u8]) -> Result<(), Error> {
        if data.len() != 512 * 1024 {
            return Err(Error::InvalidBiosSize(data.len()));
        }
        
        self.bios.load_from_buffer(data)?;
        self.reset();
        Ok(())
    }

    fn load_disc(&mut self, data: &[u8]) -> Result<(), Error> {
        self.cd.load_disc_image(data)?;
        Ok(())
    }

    fn run_frame(&mut self) -> Result<(), Error> {
        const CYCLES_PER_FRAME: u32 = 33_868_800 / 60;
        
        for _ in 0..CYCLES_PER_FRAME {
            self.run_instruction()?;
        }
        
        Ok(())
    }

    fn set_controller_state(&mut self, port: u8, state: u16) {
        if port < 2 {
            self.pad_memcard.set_digital_pad_state(port, state);
        }
    }

    fn get_display_size(&self) -> (u32, u32) {
        let display_mode = self.gpu.display_mode();
        (display_mode.width() as u32, display_mode.height() as u32)
    }

    fn get_framebuffer(&mut self, buffer: &mut [u8]) {
        let vram = self.gpu.get_vram();
        let display_mode = self.gpu.display_mode();
        let width = display_mode.width() as usize;
        let height = display_mode.height() as usize;
        
        for y in 0..height {
            for x in 0..width {
                let pixel = vram[y * 1024 + x];
                let r = ((pixel & 0x1F) << 3) as u8;
                let g = (((pixel >> 5) & 0x1F) << 3) as u8;
                let b = (((pixel >> 10) & 0x1F) << 3) as u8;
                
                let idx = (y * width + x) * 4;
                buffer[idx] = r;
                buffer[idx + 1] = g;
                buffer[idx + 2] = b;
                buffer[idx + 3] = 255;
            }
        }
    }

    fn get_audio_samples(&mut self, buffer: &mut [f32]) {
        let samples = self.spu.get_audio_buffer();
        
        for (i, &sample) in samples.iter().enumerate() {
            if i >= buffer.len() {
                break;
            }
            buffer[i] = (sample as f32) / 32768.0;
        }
    }

    fn serialize_state(&self) -> Vec<u8> {
        use serde::Serialize;
        
        let mut buffer = Vec::new();
        if let Ok(serialized) = flexbuffers::to_vec(self) {
            buffer = serialized;
        }
        buffer
    }

    fn deserialize_state(&mut self, data: &[u8]) -> Result<(), Error> {
        use serde::Deserialize;
        
        let state: Psx = flexbuffers::from_slice(data)
            .map_err(|e| Error::DeserializationError(e.to_string()))?;
        
        *self = state;
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidBiosSize(usize),
    BiosLoadError(String),
    DiscLoadError(String),
    EmulationError(String),
    DeserializationError(String),
}

impl From<crate::error::Error> for Error {
    fn from(err: crate::error::Error) -> Self {
        Error::EmulationError(format!("{:?}", err))
    }
}