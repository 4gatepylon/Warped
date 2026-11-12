//! `audio` synthesizes the demo's sound effects at startup and mixes them on SDL's audio
//! thread, panned by where they come from.
//!
//! Example:
//!     let audio: Audio = Audio::new(&sdl);
//!     audio.play(Sound::Shot, 1.0, 0.0);

use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream, AudioStreamWithCallback};

/// `SAMPLE_RATE_HZ` is the mixer's sample rate.
const SAMPLE_RATE_HZ: i32 = 48_000;
/// `MAX_VOICE_COUNT` is how many sounds can play at once.
const MAX_VOICE_COUNT: usize = 32;
/// `MASTER_GAIN` scales the whole mix.
const MASTER_GAIN: f32 = 0.55;

/// `Sound` is one of the synthesized effects.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    Shot,
    BotShot,
    Impact,
    Reload,
    Empty,
    Hurt,
    BotDeath,
    Footstep,
    HitMarker,
}

impl Sound {
    /// `ALL` lists every sound in clip order.
    const ALL: [Sound; 9] = [
        Sound::Shot,
        Sound::BotShot,
        Sound::Impact,
        Sound::Reload,
        Sound::Empty,
        Sound::Hurt,
        Sound::BotDeath,
        Sound::Footstep,
        Sound::HitMarker,
    ];
}

/// `Voice` is one playing clip.
struct Voice {
    clip: Arc<Vec<f32>>,
    position: usize,
    gain_left: f32,
    gain_right: f32,
}

/// `Mixer` is the audio-thread callback: it sums the playing voices.
struct Mixer {
    voices: Arc<Mutex<Vec<Voice>>>,
    buffer: Vec<f32>,
}

impl AudioCallback<f32> for Mixer {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let sample_count: usize = requested.max(0) as usize;
        self.buffer.clear();
        self.buffer.resize(sample_count, 0.0);
        if let Ok(mut voices) = self.voices.lock() {
            for voice in voices.iter_mut() {
                for frame in self.buffer.as_chunks_mut::<2>().0 {
                    let Some(sample) = voice.clip.get(voice.position) else {
                        break;
                    };
                    frame[0] += sample * voice.gain_left;
                    frame[1] += sample * voice.gain_right;
                    voice.position += 1;
                }
            }
            voices.retain(|voice: &Voice| voice.position < voice.clip.len());
        }
        for sample in &mut self.buffer {
            *sample = (*sample * MASTER_GAIN).tanh();
        }
        let _ = stream.put_data_f32(&self.buffer);
    }
}

/// `Audio` owns the output stream and the synthesized clips.
pub struct Audio {
    voices: Arc<Mutex<Vec<Voice>>>,
    clips: Vec<Arc<Vec<f32>>>,
    _stream: Option<AudioStreamWithCallback<Mixer>>,
}

/// `Noise` is a small deterministic white-noise generator.
struct Noise(u32);

impl Noise {
    /// `next` returns noise in [-1, 1].
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// `synthesize` renders a clip of `duration_s` from a function of time and noise.
fn synthesize(duration_s: f32, mut sample_at: impl FnMut(f32, &mut Noise) -> f32) -> Vec<f32> {
    let mut noise: Noise = Noise(0x9e37_79b9);
    let sample_count: usize = (duration_s * SAMPLE_RATE_HZ as f32) as usize;
    (0..sample_count)
        .map(|index| sample_at(index as f32 / SAMPLE_RATE_HZ as f32, &mut noise))
        .collect()
}

/// `chirp` returns a sweeping oscillator's phase-accumulating closure.
fn chirp(start_hz: f32, end_hz: f32, duration_s: f32) -> impl FnMut(f32) -> f32 {
    let mut phase: f32 = 0.0;
    move |time_s: f32| {
        let frac: f32 = (time_s / duration_s).min(1.0);
        let frequency_hz: f32 = start_hz * (end_hz / start_hz).powf(frac);
        phase = (phase + frequency_hz / SAMPLE_RATE_HZ as f32).fract();
        phase
    }
}

/// `clip_for` synthesizes one sound.
fn clip_for(sound: Sound) -> Vec<f32> {
    match sound {
        Sound::Shot => {
            let mut zap = chirp(2400.0, 140.0, 0.28);
            let mut thump = chirp(120.0, 40.0, 0.2);
            let mut low_noise: f32 = 0.0;
            synthesize(0.45, move |t, noise| {
                let zap_phase: f32 = zap(t);
                let saw: f32 = zap_phase * 2.0 - 1.0;
                let body: f32 = (thump(t) * TAU).sin() * (-t * 18.0).exp();
                low_noise += (noise.next() - low_noise) * 0.25;
                let crack: f32 = low_noise * (-t * 40.0).exp();
                ((saw * 0.5 + (zap_phase * TAU).sin()) * (-t * 9.0).exp() * 0.5
                    + body * 0.9
                    + crack * 0.8)
                    * 1.4
            })
        }
        Sound::BotShot => {
            let mut zap = chirp(1300.0, 260.0, 0.22);
            synthesize(0.3, move |t, noise| {
                let phase: f32 = zap(t);
                let square: f32 = if phase < 0.5 { 1.0 } else { -1.0 };
                (square * 0.35 + noise.next() * 0.1) * (-t * 12.0).exp()
            })
        }
        Sound::Impact => {
            let mut filtered: f32 = 0.0;
            synthesize(0.18, move |t, noise| {
                filtered += (noise.next() - filtered) * 0.4;
                filtered * (-t * 30.0).exp() * 1.2
            })
        }
        Sound::Reload => {
            let mut whine = chirp(220.0, 1400.0, 1.1);
            synthesize(1.3, move |t, noise| {
                let click = |start_s: f32| -> f32 {
                    let local_s: f32 = t - start_s;
                    if local_s < 0.0 {
                        0.0
                    } else {
                        (-local_s * 400.0).exp()
                    }
                };
                let clicks: f32 = (click(0.05) + click(0.35) + click(1.15)) * noise.next() * 1.2;
                let rise: f32 = (whine(t) * TAU).sin() * (t / 1.1).min(1.0) * 0.18;
                let fade: f32 = if t > 1.1 {
                    (-(t - 1.1) * 20.0).exp()
                } else {
                    1.0
                };
                clicks + rise * fade
            })
        }
        Sound::Empty => synthesize(0.06, |t, noise| noise.next() * (-t * 300.0).exp()),
        Sound::Hurt => {
            let mut thud = chirp(160.0, 50.0, 0.25);
            synthesize(0.3, move |t, noise| {
                ((thud(t) * TAU).sin() + noise.next() * 0.3) * (-t * 14.0).exp()
            })
        }
        Sound::BotDeath => {
            let mut fall = chirp(700.0, 50.0, 0.9);
            synthesize(1.0, move |t, noise| {
                let phase: f32 = fall(t);
                ((phase * 2.0 - 1.0) * 0.4 + noise.next() * 0.25 * (-t * 6.0).exp())
                    * (-t * 3.0).exp()
            })
        }
        Sound::Footstep => {
            let mut filtered: f32 = 0.0;
            synthesize(0.09, move |t, noise| {
                filtered += (noise.next() - filtered) * 0.08;
                filtered * (-t * 50.0).exp() * 2.5
            })
        }
        Sound::HitMarker => synthesize(0.05, |t, _| (t * 2600.0 * TAU).sin() * (-t * 80.0).exp()),
    }
}

impl Audio {
    /// `new` synthesizes every clip and opens a stereo output stream; the demo stays silent
    /// if there's no audio device.
    ///
    /// Example:
    ///     let audio: Audio = Audio::new(&platform.sdl);
    pub fn new(sdl: &sdl3::Sdl) -> Audio {
        let voices: Arc<Mutex<Vec<Voice>>> = Arc::new(Mutex::new(Vec::new()));
        let clips: Vec<Arc<Vec<f32>>> = Sound::ALL
            .iter()
            .map(|sound| Arc::new(clip_for(*sound)))
            .collect();
        let spec: AudioSpec = AudioSpec {
            freq: Some(SAMPLE_RATE_HZ),
            channels: Some(2),
            format: Some(AudioFormat::f32_sys()),
        };
        let stream: Option<AudioStreamWithCallback<Mixer>> = sdl.audio().ok().and_then(|audio| {
            let mixer: Mixer = Mixer {
                voices: voices.clone(),
                buffer: Vec::new(),
            };
            let stream = audio.open_playback_stream(&spec, mixer).ok()?;
            stream.resume().ok()?;
            Some(stream)
        });
        if stream.is_none() {
            eprintln!("no audio device; running silently");
        }
        Audio {
            voices,
            clips,
            _stream: stream,
        }
    }

    /// `play` starts a sound; `pan` runs from -1 (left) to 1 (right).
    ///
    /// Example:
    ///     audio.play(Sound::Impact, 0.6, -0.3);
    pub fn play(&self, sound: Sound, gain: f32, pan: f32) {
        let index: usize = Sound::ALL.iter().position(|s| *s == sound).unwrap_or(0);
        let angle_rad: f32 = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        if let Ok(mut voices) = self.voices.lock() {
            if voices.len() >= MAX_VOICE_COUNT {
                voices.remove(0);
            }
            voices.push(Voice {
                clip: self.clips[index].clone(),
                position: 0,
                gain_left: gain * angle_rad.cos(),
                gain_right: gain * angle_rad.sin(),
            });
        }
    }
}
