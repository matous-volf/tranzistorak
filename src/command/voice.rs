use circular_queue::CircularQueue;
use ct2rs::{Whisper, WhisperOptions};
use futures::{FutureExt, Stream, StreamExt, pin_mut};
use log::error;
use serenity::{
    Result as SerenityResult, async_trait,
    client::{Client, Context},
    framework::{
        StandardFramework,
        standard::{
            Args, CommandResult, Configuration,
            macros::{command, group},
        },
    },
    model::{channel::Message, gateway::Ready, id::ChannelId},
    prelude::{GatewayIntents, Mentionable},
};
use songbird::driver::SampleRate;
use songbird::{
    CoreEvent, Event, EventContext, EventHandler, SerenityInit,
    driver::DecodeMode,
    model::{
        id::UserId,
        payload::{ClientDisconnect, Speaking},
    },
    packet::Packet,
};
use std::collections::VecDeque;
use std::iter::{Rev, Take};
use std::pin::Pin;
use std::task::Poll;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{
    env, iter, sync,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread, vec,
};
use tokio::sync::{Mutex, MutexGuard};
use unwrap_or_log::LogError;

pub(crate) const SAMPLE_RATE: SampleRate = SampleRate::Hz16000;
pub(crate) const SAMPLE_RATE_RAW: u32 = 16_000;

#[derive(Clone)]
pub(crate) struct Handler {
    receiver: Arc<Receiver>,
}

struct Receiver {
    buffer: Mutex<CircularQueue<i16>>,
    current_silence_sample_count: Mutex<usize>,
    transcription_queue: Mutex<VecDeque<Vec<f32>>>,
    whisper: Whisper,
}

impl Receiver {
    async fn transcribe_from_queue(&self) {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            // println!(
            //     "{} {}",
            //     self.current_silence_sample_count.lock().await,
            //     self.buffer.lock().await.len()
            // );

            let decoded_voice = match self.transcription_queue.lock().await.pop_front() {
                None => continue,
                Some(voice_stream) => voice_stream,
            };

            println!("processing");
            // let spec = hound::WavSpec {
            //     channels: 1,
            //     sample_rate: SAMPLE_RATE_RAW,
            //     bits_per_sample: 16,
            //     sample_format: hound::SampleFormat::Int,
            // };
            // let mut writer = hound::WavWriter::create(
            //     SystemTime::now()
            //         .duration_since(UNIX_EPOCH)
            //         .unwrap()
            //         .as_secs()
            //         .to_string(),
            //     spec,
            // )
            // .unwrap();
            // for sample in voice_stream.clone() {
            //     writer.write_sample(sample).unwrap();
            // }
            // writer.finalize().unwrap();

            let text = match self
                .whisper
                .generate(
                    decoded_voice.as_slice(),
                    Some("cs"),
                    false,
                    &WhisperOptions::default(),
                )
                .map(|parts| parts.concat())
            {
                Err(error) => {
                    error!("{error}");
                    continue;
                }
                Ok(text) => text,
            };

            println!("{}", text);
        }
    }
}

impl Handler {
    /// Determined by the Discord API.
    const EVENT_FIRE_INTERVAL: Duration = Duration::from_millis(20);
    const SILENCE_SAMPLE_THRESHOLD: i16 = i16::MAX / 100;
    const SILENCE_SEPARATOR_DURATION: Duration = Duration::from_secs(3);
    const TRANSCRIPTION_DURATION: Duration = Duration::from_secs(10);

    pub async fn new() -> anyhow::Result<Self> {
        let receiver = Arc::new(Receiver {
            buffer: Mutex::new(CircularQueue::with_capacity(
                ((Self::SILENCE_SEPARATOR_DURATION + Self::TRANSCRIPTION_DURATION).as_secs_f64()
                    * SAMPLE_RATE_RAW as f64) as usize,
            )),
            current_silence_sample_count: Mutex::new(0),
            transcription_queue: Mutex::new(VecDeque::new()),
            whisper: Whisper::new(
                "transcription_model",
                ct2rs::Config::default(),
            )?,
        });

        {
            let receiver = receiver.clone();
            tokio::spawn(async move {
                receiver.transcribe_from_queue().await;
            });
        }

        Ok(Self { receiver })
    }

    fn mix_audio_sources<'a>(sources: impl AsRef<[&'a Vec<i16>]>) -> Vec<i16> {
        let sources = sources.as_ref();
        let source_count = sources.len() as i16;

        let mut samples_by_source = sources
            .iter()
            .map(|source| source.iter())
            .collect::<Vec<_>>();

        let max_source_length = sources.iter().map(|source| source.len()).max().unwrap_or(0);
        iter::repeat_with(|| {
            samples_by_source
                .iter_mut()
                .map(|input_buffer| input_buffer.next().unwrap_or(&0) / source_count)
                .sum()
        })
        .take(max_source_length)
        .collect()
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn act(&self, context: &EventContext<'_>) -> Option<Event> {
        let voice_tick = match context {
            EventContext::VoiceTick(voice_tick) => voice_tick,
            _ => return None,
        };

        let (decoded_voice_length, decoded_voice_is_silent) = match voice_tick.speaking.is_empty() {
            true => {
                let mut buffer = self.receiver.buffer.lock().await;
                let silence_length = match buffer.is_empty() {
                    true => 0,
                    false => {
                        let silence_length = (Self::EVENT_FIRE_INTERVAL.as_secs_f64()
                            * SAMPLE_RATE_RAW as f64)
                            as usize;
                        for sample in iter::repeat_n(0, silence_length) {
                            buffer.push(sample);
                        }
                        silence_length
                    }
                };

                (silence_length, true)
            }
            false => {
                let decoded_voices = voice_tick
                    .speaking
                    .values()
                    .filter_map(|voice_data| {
                        let decoded_voice = voice_data.decoded_voice.as_ref();
                        if decoded_voice.is_none() {
                            error!("The voice tick data is missing the decoded voice.");
                        }
                        decoded_voice
                    })
                    .collect::<Vec<_>>();

                let mixed_decoded_voice = Self::mix_audio_sources(decoded_voices).into_iter();
                let mixed_decoded_voice_length = mixed_decoded_voice.len();

                let mut mixed_decoded_voice_is_silent = true;
                let mut buffer = self.receiver.buffer.lock().await;
                for sample in mixed_decoded_voice {
                    if sample.abs() > Self::SILENCE_SAMPLE_THRESHOLD {
                        mixed_decoded_voice_is_silent = false;
                    }
                    buffer.push(sample);
                }

                (mixed_decoded_voice_length, mixed_decoded_voice_is_silent)
            }
        };

        {
            let mut current_silence_sample_count =
                self.receiver.current_silence_sample_count.lock().await;
            match decoded_voice_is_silent {
                false => *current_silence_sample_count = 0,
                true => {
                    *current_silence_sample_count += decoded_voice_length;
                    if *current_silence_sample_count
                        > (Self::SILENCE_SEPARATOR_DURATION.as_secs_f64() * SAMPLE_RATE_RAW as f64)
                            as usize
                    {
                        let current_silence_sample_count = {
                            let clone = *current_silence_sample_count;
                            *current_silence_sample_count = 0;
                            drop(current_silence_sample_count);
                            clone
                        };

                        let voice_stream = {
                            let mut buffer = self.receiver.buffer.lock().await;
                            let voice_stream = buffer
                                .clone()
                                .into_vec()
                                .into_iter()
                                .rev()
                                .take(buffer.len() - current_silence_sample_count)
                                .map(cpal::Sample::to_float_sample)
                                .collect();
                            buffer.clear();
                            voice_stream
                        };
                        self.receiver
                            .transcription_queue
                            .lock()
                            .await
                            .push_back(voice_stream);
                    }
                }
            }
        }

        None
    }
}
