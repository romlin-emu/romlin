//! The microphone offered to cores that take one, such as SingStar's. The device is one USB input
//! matched by name. Its left channel reaches the first handle a core opens and its right channel
//! the second; a mono device feeds both.

use cubeb::{
    ChannelLayout, Context, DeviceId, DeviceState, DeviceType, Frame, MonoFrame, SampleFormat,
    StereoFrame, Stream, StreamBuilder, StreamParamsBuilder, StreamParamsRef,
};
use ringbuf::traits::{Consumer, Producer};
use ringbuf::{Cons, HeapRb, Prod};
use romlin_libretro as lr;
use std::cell::RefCell;
use std::ffi::CString;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// How much captured audio each channel buffers while the core is not reading it.
const RING_MS: usize = 200;
/// One ring and one handle per channel of a stereo device.
const CHANNELS: usize = 2;

type Ring = Arc<HeapRb<i16>>;

/// Offers the input named `product` to cores. Nothing is offered unless the name matches
/// exactly one enabled input; the core then hears silence.
pub fn install(product: &str) {
    match open(product) {
        Ok(provider) => lr::install_microphone_provider(Box::new(provider)),
        Err(e) => warn!("microphone unavailable, so the game will hear silence: {e:#}"),
    }
}

/// Lowercase ASCII letters and digits, so `USB MIC` matches `usb_mic`, `USBMIC` and `usb-mic`.
fn normalise(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Whether an input's friendly name or device id contains the product string.
pub fn names_product(product: &str, friendly: Option<&str>, device_id: Option<&str>) -> bool {
    let product = normalise(product);
    !product.is_empty()
        && [friendly, device_id]
            .into_iter()
            .flatten()
            .any(|name| normalise(name).contains(&product))
}

fn open(product: &str) -> anyhow::Result<Provider> {
    let name = CString::new("Romlin")?;
    let ctx = Context::init(Some(name.as_c_str()), None)?;
    let devices = ctx.enumerate_devices(DeviceType::INPUT)?;
    let found: Vec<_> = devices
        .iter()
        .filter(|d| {
            d.state() == DeviceState::Enabled
                && names_product(product, d.friendly_name(), d.device_id())
        })
        .collect();
    let device = match found.as_slice() {
        [device] => *device,
        _ => {
            let inputs: Vec<&str> = devices
                .iter()
                .map(|d| d.friendly_name().or(d.device_id()).unwrap_or("unnamed"))
                .collect();
            anyhow::bail!(
                "expected one enabled input named {product:?}, found {}; inputs: {}",
                found.len(),
                inputs.join(", ")
            );
        }
    };
    let channels: u32 = if device.max_channels() >= 2 { 2 } else { 1 };
    let rate = device.default_rate();
    let params = StreamParamsBuilder::new()
        .format(SampleFormat::S16NE)
        .rate(rate)
        .channels(channels)
        .layout(if channels == 2 {
            ChannelLayout::STEREO
        } else {
            ChannelLayout::MONO
        })
        .take();
    let latency = ctx.min_latency(&params)?;

    let capacity = rate as usize * RING_MS / 1000;
    let (producers, consumers): (Vec<_>, Vec<_>) = (0..CHANNELS)
        .map(|_| {
            let rb = Arc::new(HeapRb::<i16>::new(capacity));
            (Prod::new(rb.clone()), Cons::new(rb))
        })
        .unzip();
    let mut split = Split {
        rings: producers,
        channels: channels as usize,
    };
    let device_id = device.devid();
    let stream = if channels == 2 {
        Input::Stereo(start_input(&ctx, device_id, &params, latency, move |s| {
            split.push(s);
        })?)
    } else {
        Input::Mono(start_input(&ctx, device_id, &params, latency, move |s| {
            split.push(s);
        })?)
    };
    info!(
        device = device.friendly_name(),
        channels, rate, "microphone input selected"
    );
    drop(devices);

    Ok(Provider {
        capture: Rc::new(RefCell::new(Capture {
            running: 0,
            free: consumers.into_iter().map(Some).collect(),
            stream,
            _ctx: ctx,
        })),
        rate,
    })
}

/// Opens an input stream whose callback passes its interleaved samples to `sink`.
fn start_input<F: Frame + 'static>(
    ctx: &Context,
    device: DeviceId,
    params: &StreamParamsRef,
    latency: u32,
    mut sink: impl FnMut(&[i16]) + Send + Sync + 'static,
) -> anyhow::Result<Stream<F>> {
    let mut builder = StreamBuilder::<F>::new();
    builder
        .name("Romlin microphone")
        .input(device, params)
        .latency(latency)
        .data_callback(move |input: &[F], _output: &mut [F]| -> isize {
            // Both frame types are runs of i16 samples with no padding.
            let count = std::mem::size_of_val(input) / std::mem::size_of::<i16>();
            let samples =
                unsafe { std::slice::from_raw_parts(input.as_ptr().cast::<i16>(), count) };
            sink(samples);
            input.len() as isize
        })
        .state_callback(|state| debug!("microphone state: {state:?}"));
    Ok(builder.init(ctx)?)
}

enum Input {
    Stereo(Stream<StereoFrame<i16>>),
    Mono(Stream<MonoFrame<i16>>),
}

/// The device's stream, shared by the handles that read it. It runs while any handle is active.
struct Capture {
    running: usize,
    /// Each channel's ring while no handle holds it. Cores close their handles when the game stops
    /// listening and open them again later, so a closed handle's ring comes back here.
    free: Vec<Option<Cons<Ring>>>,
    stream: Input,
    // Fields drop in order, so the context outlives the stream that runs on it.
    _ctx: Context,
}

impl Capture {
    fn retain(&mut self, active: bool) -> bool {
        if active {
            if self.running == 0 {
                let started = match &self.stream {
                    Input::Stereo(s) => s.start(),
                    Input::Mono(s) => s.start(),
                };
                if let Err(e) = started {
                    warn!("microphone didn't start: {e}");
                    return false;
                }
            }
            self.running += 1;
        } else if self.running > 0 {
            self.running -= 1;
            if self.running == 0 {
                let _ = match &self.stream {
                    Input::Stereo(s) => s.stop(),
                    Input::Mono(s) => s.stop(),
                };
            }
        }
        true
    }
}

/// Deinterleaves captured frames into one ring per handle. A mono device fills every ring.
struct Split {
    rings: Vec<Prod<Ring>>,
    channels: usize,
}

impl Split {
    fn push(&mut self, samples: &[i16]) {
        for frame in samples.chunks_exact(self.channels) {
            for (i, ring) in self.rings.iter_mut().enumerate() {
                // A full ring drops the newest sample; the core reads at its own pace.
                let _ = ring.try_push(frame[i.min(self.channels - 1)]);
            }
        }
    }
}

struct Provider {
    capture: Rc<RefCell<Capture>>,
    rate: u32,
}

impl lr::MicrophoneProvider for Provider {
    /// The lowest free channel, so the first handle open is the left one.
    fn open(&mut self, _rate: u32) -> Option<Box<dyn lr::Microphone>> {
        let (channel, mut ring) = self
            .capture
            .borrow_mut()
            .free
            .iter_mut()
            .enumerate()
            .find_map(|(i, ring)| ring.take().map(|ring| (i, ring)))?;
        ring.clear();
        Some(Box::new(Handle {
            capture: Rc::downgrade(&self.capture),
            channel,
            ring: Some(ring),
            rate: self.rate,
            active: false,
        }))
    }
}

/// One channel of the microphone, as a core sees it. Once the provider is gone, a handle reads
/// nothing and starts nothing.
struct Handle {
    capture: Weak<RefCell<Capture>>,
    channel: usize,
    /// Always `Some` until the handle drops and gives it back.
    ring: Option<Cons<Ring>>,
    rate: u32,
    active: bool,
}

impl lr::Microphone for Handle {
    fn sample_rate(&self) -> u32 {
        self.rate
    }

    fn set_active(&mut self, active: bool) -> bool {
        if active == self.active {
            return true;
        }
        let Some(capture) = self.capture.upgrade() else {
            return false;
        };
        if !capture.borrow_mut().retain(active) {
            return false;
        }
        self.active = active;
        true
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn read(&mut self, samples: &mut [i16]) -> Option<usize> {
        if !self.active {
            return None;
        }
        self.ring.as_mut().map(|ring| ring.pop_slice(samples))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(capture) = self.capture.upgrade() {
            let mut capture = capture.borrow_mut();
            if self.active {
                capture.retain(false);
            }
            capture.free[self.channel] = self.ring.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_matches_pulseaudio_and_wasapi_names() {
        let pulse_source = "alsa_input.usb-Generic_USBMIC_Serial_0001-00.analog-stereo";
        let pulse_description = "USBMIC Serial Analog Stereo";
        let wasapi = "Microphone (USBMIC Serial# 0001)";
        assert!(names_product(
            "USBMIC Serial",
            Some(pulse_description),
            None
        ));
        assert!(names_product("USBMIC Serial", None, Some(pulse_source)));
        assert!(names_product("usbmic-serial", Some(wasapi), None));
        assert!(names_product(
            "USB MIC serial",
            Some(pulse_description),
            None
        ));
    }

    #[test]
    fn other_inputs_and_empty_names_do_not_match() {
        let built_in = "Built-in Audio Analog Stereo";
        let built_in_id = "alsa_input.pci-0000_00_1f.3.analog-stereo";
        assert!(!names_product(
            "USBMIC Serial",
            Some(built_in),
            Some(built_in_id)
        ));
        assert!(!names_product("", Some(built_in), Some(built_in_id)));
        assert!(!names_product("USBMIC", None, None));
    }

    #[test]
    fn a_stereo_device_feeds_one_channel_to_each_ring_and_a_mono_device_to_both() {
        let ring = |capacity| {
            let rb = Arc::new(HeapRb::<i16>::new(capacity));
            (Prod::new(rb.clone()), Cons::new(rb))
        };
        let (left_in, mut left) = ring(8);
        let (right_in, mut right) = ring(8);
        let mut stereo = Split {
            rings: vec![left_in, right_in],
            channels: 2,
        };
        stereo.push(&[1, -1, 2, -2]);
        let mut out = [0i16; 4];
        assert_eq!(left.pop_slice(&mut out), 2);
        assert_eq!(&out[..2], &[1, 2]);
        assert_eq!(right.pop_slice(&mut out), 2);
        assert_eq!(&out[..2], &[-1, -2]);

        let (mono_left_in, mut mono_left) = ring(8);
        let (mono_right_in, mut mono_right) = ring(8);
        let mut mono = Split {
            rings: vec![mono_left_in, mono_right_in],
            channels: 1,
        };
        mono.push(&[7, 8]);
        assert_eq!(mono_left.pop_slice(&mut out), 2);
        assert_eq!(&out[..2], &[7, 8]);
        assert_eq!(mono_right.pop_slice(&mut out), 2);
        assert_eq!(&out[..2], &[7, 8]);
    }
}
