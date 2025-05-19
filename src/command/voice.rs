use amplify_derive::Display;
use circular_queue::CircularQueue;
use ct2rs::{Whisper, WhisperOptions};
use log::error;
use serenity::all::GuildId;
use serenity::async_trait;
use songbird::driver::SampleRate;
use songbird::events::context_data::VoiceTick;
use std::collections::{HashMap, VecDeque};
use std::iter;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::fs;
use tokio::sync::{Mutex, RwLock};

pub(crate) const SAMPLE_RATE: SampleRate = SampleRate::Hz16000;
pub(crate) const SAMPLE_RATE_RAW: u32 = 16_000;

struct TranscriptionQueueItem {
    samples: Vec<f32>,
    guild_id: GuildId,
}

impl TranscriptionQueueItem {
    pub(crate) fn new(samples: Vec<f32>, guild_id: GuildId) -> Self {
        Self { samples, guild_id }
    }
}

#[async_trait]
pub(crate) trait VoiceTranscribedCallback: Send + Sync + Clone + 'static {
    async fn on_voice_transcribed(&self, guild_id: GuildId, text: String);
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum TranscriptorCreationError {
    HuggingFaceHubApiCreation(hf_hub::api::tokio::ApiError),
    ModelFileDownload(hf_hub::api::tokio::ApiError),
    ModelFileMove(std::io::Error),
    ModelCreation(anyhow::Error),
}

pub(crate) struct Transcriptor<V: VoiceTranscribedCallback> {
    buffers: RwLock<HashMap<GuildId, Mutex<CircularQueue<i16>>>>,
    current_silence_sample_count: Mutex<usize>,
    queue: Mutex<VecDeque<TranscriptionQueueItem>>,
    model: Whisper,
    voice_transcribed_callback: Mutex<Option<V>>,
}

impl<V: VoiceTranscribedCallback> Transcriptor<V> {
    const TRANSCRIPTION_MODEL_DIRECTORY: &'static str = "transcription_model";

    /// Determined by the Discord API.
    const EVENT_FIRE_INTERVAL: Duration = Duration::from_millis(20);
    const SILENCE_SAMPLE_THRESHOLD: i16 = i16::MAX / 50;
    const SILENCE_SEPARATOR_DURATION: Duration = Duration::from_secs(3);
    const TRANSCRIPTION_DURATION: Duration = Duration::from_secs(7);

    pub(crate) async fn new(
        voice_transcribed_callback: Option<V>,
    ) -> Result<Arc<Self>, TranscriptorCreationError> {
        let new = Arc::new(Self {
            voice_transcribed_callback: Mutex::new(voice_transcribed_callback),
            buffers: RwLock::new(HashMap::new()),
            current_silence_sample_count: Mutex::new(0),
            queue: Mutex::new(VecDeque::new()),
            model: Self::create_transcription_model().await?,
        });

        {
            let new = new.clone();
            tokio::spawn(async move {
                new.transcribe_from_queue().await;
            });
        }

        Ok(new)
    }

    async fn create_transcription_model() -> Result<Whisper, TranscriptorCreationError> {
        const MODEL_NAME: &str = "Systran/faster-whisper-small";
        const MODEL_FILE_NAMES: [&str; 4] = [
            "model.bin",
            "config.json",
            "tokenizer.json",
            "vocabulary.txt",
        ];
        const PREPROCESSOR_CONFIG_MODEL_NAME: &str = "openai/whisper-small";
        const PREPROCESSOR_CONFIG_FILE_NAMES: [&str; 1] = ["preprocessor_config.json"];

        let hugging_face_api = hf_hub::api::tokio::Api::new()
            .map_err(TranscriptorCreationError::HuggingFaceHubApiCreation)?;

        let model_repository = hugging_face_api.model(MODEL_NAME.to_owned());
        for model_file_name in MODEL_FILE_NAMES {
            Self::download_transcription_model_file(&model_repository, model_file_name).await?;
        }
        let preprocessor_config_repository =
            hugging_face_api.model(PREPROCESSOR_CONFIG_MODEL_NAME.to_owned());
        for preprocessor_config_file_name in PREPROCESSOR_CONFIG_FILE_NAMES {
            Self::download_transcription_model_file(
                &preprocessor_config_repository,
                preprocessor_config_file_name,
            )
            .await?;
        }

        Whisper::new(
            Self::TRANSCRIPTION_MODEL_DIRECTORY,
            ct2rs::Config::default(),
        )
        .map_err(TranscriptorCreationError::ModelCreation)
    }

    async fn download_transcription_model_file(
        repository: &hf_hub::api::tokio::ApiRepo,
        file_name: impl AsRef<str>,
    ) -> Result<(), TranscriptorCreationError> {
        let destination =
            PathBuf::from_iter([Self::TRANSCRIPTION_MODEL_DIRECTORY, file_name.as_ref()]);

        if destination.exists() {
            return Ok(());
        }

        let model_file_downloaded = repository
            .get(file_name.as_ref())
            .await
            .map_err(TranscriptorCreationError::ModelFileDownload)?;
        fs::copy(model_file_downloaded, destination)
            .await
            .map_err(TranscriptorCreationError::ModelFileMove)?;

        Ok(())
    }

    pub(crate) async fn process_voice_tick(&self, guild_id: GuildId, voice_tick: &VoiceTick) {
        if !self.buffers.read().await.contains_key(&guild_id) {
            self.buffers.write().await.insert(
                guild_id,
                Mutex::new(CircularQueue::with_capacity(
                    ((Self::SILENCE_SEPARATOR_DURATION + Self::TRANSCRIPTION_DURATION)
                        .as_secs_f64()
                        * SAMPLE_RATE_RAW as f64) as usize,
                )),
            );
        }
        let buffers = self.buffers.read().await;
        let buffer = buffers.get(&guild_id).unwrap();

        let (decoded_voice_length, decoded_voice_is_silent) = if voice_tick.speaking.is_empty() {
            let mut buffer = buffer.lock().await;
            let silence_length = match buffer.is_empty() {
                true => 0,
                false => {
                    let silence_length =
                        (Self::EVENT_FIRE_INTERVAL.as_secs_f64() * SAMPLE_RATE_RAW as f64) as usize;
                    for sample in iter::repeat_n(0, silence_length) {
                        buffer.push(sample);
                    }
                    silence_length
                }
            };

            (silence_length, true)
        } else {
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
            let mut buffer = buffer.lock().await;
            for sample in mixed_decoded_voice {
                if sample.abs() > Self::SILENCE_SAMPLE_THRESHOLD {
                    mixed_decoded_voice_is_silent = false;
                }
                buffer.push(sample);
            }

            (mixed_decoded_voice_length, mixed_decoded_voice_is_silent)
        };

        {
            let mut current_silence_sample_count = self.current_silence_sample_count.lock().await;
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

                        let queue_item = {
                            let mut buffer = buffer.lock().await;
                            let queue_item = TranscriptionQueueItem::new(
                                buffer
                                    .clone()
                                    .into_vec()
                                    .into_iter()
                                    .rev()
                                    .take(buffer.len() - current_silence_sample_count)
                                    .map(cpal::Sample::to_float_sample)
                                    .collect(),
                                guild_id,
                            );
                            buffer.clear();
                            queue_item
                        };
                        self.queue.lock().await.push_back(queue_item);
                    }
                }
            }
        }
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

    async fn transcribe_from_queue(&self) {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let queue_item = match self.queue.lock().await.pop_front() {
                None => continue,
                Some(queue_item) => queue_item,
            };

            if self.voice_transcribed_callback.lock().await.is_none() {
                return;
            }

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
                .model
                .generate(
                    queue_item.samples.as_slice(),
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

            match self.voice_transcribed_callback.lock().await.as_ref() {
                None => return,
                Some(voice_transcribed_callback) => {
                    let voice_transcribed_callback = voice_transcribed_callback.clone();
                    tokio::spawn(async move {
                        voice_transcribed_callback
                            .on_voice_transcribed(queue_item.guild_id, text)
                            .await
                    });
                }
            };
        }
    }

    pub(crate) async fn set_voice_transcribed_callback(
        &self,
        voice_transcribed_callback: Option<V>,
    ) {
        *self.voice_transcribed_callback.lock().await = voice_transcribed_callback;
    }
}
