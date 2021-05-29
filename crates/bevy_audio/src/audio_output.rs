use crate::{Audio, AudioSource, Decodable};
use bevy_asset::{Asset, Assets};
use bevy_ecs::world::World;
use bevy_utils::tracing::warn;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::{cell::RefCell, marker::PhantomData};

/// Used internally to play audio on the current "audio device"
pub struct AudioOutput<P = AudioSource>
where
    P: Decodable,
{
    stream: RefCell<Option<(OutputStream, OutputStreamHandle)>>,
    phantom: PhantomData<P>,
}

impl<P> Default for AudioOutput<P>
where
    P: Decodable,
{
    fn default() -> Self {
        Self {
            stream: RefCell::new(None),
            phantom: PhantomData,
        }
    }
}

impl<P> AudioOutput<P>
where
    P: Asset + Decodable,
    <P as Decodable>::Decoder: rodio::Source + Send + Sync,
    <<P as Decodable>::Decoder as Iterator>::Item: rodio::Sample + Send + Sync,
{
    fn init_device(&self) {
        if let Ok(mut cell) = self.stream.try_borrow_mut() {
            if let Ok((stream, stream_handle)) = OutputStream::try_default() {
                cell.replace((stream, stream_handle));
            } else {
                warn!("No audio device found.");
            }
        }
    }

    fn play_source(&self, audio_source: &P) {
        for _ in 0..2 {
            if let Ok(stream) = self.stream.try_borrow() {
                if let Some((_, stream_handle)) = stream.as_ref() {
                    if let Ok(sink) = Sink::try_new(stream_handle) {
                        sink.append(audio_source.decoder());
                        sink.detach();

                        // Playback successful, break out of loop.
                        break;
                    }
                }
            }

            // Playback failed, try to initialize audio device.
            self.init_device();
        }
    }

    fn try_play_queued(&self, audio_sources: &Assets<P>, audio: &mut Audio<P>) {
        let mut queue = audio.queue.write();
        let len = queue.len();
        let mut i = 0;
        while i < len {
            let audio_source_handle = queue.pop_back().unwrap();
            if let Some(audio_source) = audio_sources.get(&audio_source_handle) {
                self.play_source(audio_source);
            } else {
                // audio source hasn't loaded yet. add it back to the queue
                queue.push_front(audio_source_handle);
            }
            i += 1;
        }
    }
}

/// Plays audio currently queued in the [Audio] resource through the [AudioOutput] resource
pub fn play_queued_audio_system<P: Asset>(world: &mut World)
where
    P: Decodable,
    <P as Decodable>::Decoder: rodio::Source + Send + Sync,
    <<P as Decodable>::Decoder as Iterator>::Item: rodio::Sample + Send + Sync,
{
    let world = world.cell();
    let audio_output = world.get_non_send::<AudioOutput<P>>().unwrap();
    let mut audio = world.get_resource_mut::<Audio<P>>().unwrap();

    if let Some(audio_sources) = world.get_resource::<Assets<P>>() {
        audio_output.try_play_queued(&*audio_sources, &mut *audio);
    };
}
