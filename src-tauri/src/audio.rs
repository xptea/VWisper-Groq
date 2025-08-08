use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{AppHandle, Emitter};
use serde_json::json;
use hound::{WavWriter, WavSpec};

const TARGET_SAMPLE_RATE: u32 = 16000;

pub struct AudioProcessor {
    buffer: Arc<Mutex<Vec<f32>>>,
    wav_writer: Option<WavWriter<std::io::BufWriter<std::fs::File>>>,
    is_recording: Arc<Mutex<bool>>,
    downsample_ratio: f32,
}

impl AudioProcessor {
    pub fn new(original_sample_rate: u32) -> Self {
        let downsample_ratio = if original_sample_rate != TARGET_SAMPLE_RATE {
            original_sample_rate as f32 / TARGET_SAMPLE_RATE as f32
        } else {
            1.0
        };

        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            wav_writer: None,
            is_recording: Arc::new(Mutex::new(false)),
            downsample_ratio,
        }
    }

    pub fn start_recording(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let spec = WavSpec {
            channels: 1,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let temp_dir = std::env::temp_dir();
        let filename = temp_dir.join("vwisper_audio_latest.wav");
        
        self.wav_writer = Some(WavWriter::create(&filename, spec)?);
        *self.is_recording.lock().unwrap() = true;
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(writer) = self.wav_writer.take() {
            writer.finalize()?;
            *self.is_recording.lock().unwrap() = false;
        }
        Ok(())
    }

    pub fn process_audio(&mut self, samples: &[f32]) -> Vec<f32> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.extend_from_slice(samples);

        if self.downsample_ratio != 1.0 {
            let mut downsampled = Vec::new();
            let mut index = 0.0;
            
            while index < buffer.len() as f32 {
                let sample_index = index as usize;
                if sample_index < buffer.len() {
                    downsampled.push(buffer[sample_index]);
                }
                index += self.downsample_ratio;
            }
            
            buffer.clear();
            downsampled
        } else {
            let output = buffer.clone();
            buffer.clear();
            output
        }
    }

    pub fn write_samples(&mut self, samples: &[f32]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(writer) = &mut self.wav_writer {
            for &sample in samples {
                let sample_i16 = (sample * i16::MAX as f32) as i16;
                writer.write_sample(sample_i16)?;
            }
        }
        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap()
    }
}

static AUDIO_PROCESSOR: std::sync::OnceLock<Arc<Mutex<Option<AudioProcessor>>>> = std::sync::OnceLock::new();

pub fn get_audio_processor() -> Arc<Mutex<Option<AudioProcessor>>> {
    AUDIO_PROCESSOR.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

pub fn start_audio_capture(app_handle: AppHandle) {
    thread::spawn(move || {
        let host = cpal::default_host();

        let device = match host.default_input_device() {
            Some(device) => device,
            None => {
                eprintln!("No default input device found");
                return;
            }
        };

        let config = match device.default_input_config() {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to get default input config: {}", e);
                return;
            }
        };

        let original_sample_rate = config.sample_rate().0;
        let audio_processor = AudioProcessor::new(original_sample_rate);
        
        let processor_arc = get_audio_processor();
        *processor_arc.lock().unwrap() = Some(audio_processor);

        let app_handle_stream = app_handle.clone();
        let stream = match device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let samples: Vec<f32> = data.to_vec();
                
                let rms = (samples.iter().map(|&x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
                let normalized = (rms * 10.0).min(1.0);
                
                let bar_values = vec![normalized; 10];
                let _ = app_handle_stream.emit_to("main", "audio-data", json!({
                    "samples": bar_values
                }));

                if let Some(processor) = &mut *processor_arc.lock().unwrap() {
                    let downsampled = processor.process_audio(&samples);
                    
                    if processor.is_recording() && !downsampled.is_empty() {
                        if let Err(e) = processor.write_samples(&downsampled) {
                            eprintln!("Failed to write samples: {}", e);
                        }
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None
        ) {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("Failed to build audio stream: {}", e);
                return;
            }
        };

        if let Err(e) = stream.play() {
            eprintln!("Failed to play audio stream: {}", e);
            return;
        }

        loop {
            thread::sleep(Duration::from_millis(100));
        }
    });
}

pub fn start_recording() -> Result<(), Box<dyn std::error::Error>> {
    let processor_arc = get_audio_processor();
    let mut guard = processor_arc.lock().unwrap();
    match guard.as_mut() {
        Some(processor) => {
            processor.start_recording()?;
            Ok(())
        }
        None => Err("Audio input not initialized".into()),
    }
}

pub fn stop_recording() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(processor) = &mut *get_audio_processor().lock().unwrap() {
        processor.stop_recording()?;
    }
    Ok(())
}

pub fn is_recording() -> bool {
    if let Some(processor) = &*get_audio_processor().lock().unwrap() {
        processor.is_recording()
    } else {
        false
    }
} 

pub fn is_ready() -> bool {
    get_audio_processor().lock().unwrap().is_some()
}