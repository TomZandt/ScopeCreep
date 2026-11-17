//! ColorChecker test frames (ticket #52), proven at the Feed seam: a chart in a Timeline Colour Space goes in
//! through the synthetic writer with a Colour Setup (the Sender's manual override travels in the Feed), the app's
//! Feed monitor hands it to the GPU exactly as the app does, and the test looks at what a colourist would see: the
//! waveform of the converted picture. Expected values come from the chart code and the f64 published maths in the
//! accuracy crate (the transforms proven against Resolve's renders in #10), never from the shader. Needs a GPU.
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::monitor::Monitor;
use scopecreep::output_conversion::{self, OutputConversion};
use scopecreep::waveform::{LumaWeights, Waveform, WaveformCounts};
use scopecreep_accuracy::aces2::AcesOutputTransform;
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_accuracy::colour::{REC709, luma_weights};
use scopecreep_accuracy::pipeline::Pipeline;
use scopecreep_feed::{FeedWriter, FrameMeta, Provenance, SentColourSetup, now_ns};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

const MS: u64 = 1_000_000;

/// One GPU device for all the tests, used by one test at a time: the graphics memory is shared with Resolve.
fn gpu() -> (MutexGuard<'static, ()>, wgpu::Device, wgpu::Queue) {
    static GPU: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (device, queue) = GPU.get_or_init(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .expect("these tests need a GPU");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("colorchecker tests"), ..Default::default() })).unwrap()
    });
    (turn, device.clone(), queue.clone())
}

/// A stand-in Resolve and Sender (the synthetic writer), the app's Feed monitor, and the app's GPU side of the
/// Feed: upload, Output Conversion, waveform, as `main.rs` wires them.
struct Bench {
    path: PathBuf,
    writer: FeedWriter,
    monitor: Monitor,
    uploader: FrameUploader,
    conversion: OutputConversion,
    waveform: Waveform,
    now: u64,
    frame: f64,
    _gpu_turn: MutexGuard<'static, ()>,
}

impl Bench {
    /// The Sender node says the project is `colour_setup` (protocol 1.1 `key=value` lines).
    fn new(name: &str, colour_setup: &str) -> Bench {
        let path = PathBuf::from(format!("/dev/shm/scopecreep-colorchecker-test-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_file(&path);
        let (turn, device, queue) = gpu();
        let mut writer = FeedWriter::create(&path, 4, 1920 * 1080 * 16).unwrap();
        writer.set_colour_setup(Some(SentColourSetup::new(Provenance::Manual, colour_setup)));
        let mut b = Bench {
            monitor: Monitor::new(&path),
            uploader: FrameUploader::new(&device, &queue),
            conversion: OutputConversion::new(&device, &queue),
            waveform: Waveform::new(&device, &queue, wgpu::TextureFormat::Rgba8Unorm),
            path,
            writer,
            now: now_ns(),
            frame: 86400.0,
            _gpu_turn: turn,
        };
        b.poll();
        b
    }

    fn poll(&mut self) {
        let (uploader, conversion, waveform) = (&mut self.uploader, &mut self.conversion, &self.waveform);
        self.monitor.poll(self.now, &mut |frame| {
            uploader.upload(frame, |encoder, gpu_frame| {
                let shown = conversion.convert(encoder, gpu_frame);
                waveform.measure(encoder, shown);
            })
        });
        // As the app does after every poll: measure in the Measured Space, and follow the Colour Setup.
        let setup = self.monitor.status().colour_setup().map(|c| c.setup);
        self.waveform.set_measurement(output_conversion::measurement(self.monitor.status().colour_setup(), LumaWeights::FromPrimaries));
        let waveform = &self.waveform;
        self.conversion.follow(setup.as_ref(), self.uploader.frame(), |encoder, shown| waveform.measure(encoder, shown));
    }

    /// Resolve plays one frame (RGBA, bottom row first, as the Sender writes it) and the app has a moment to
    /// take it.
    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let meta = FrameMeta { width, height, frame_time: self.frame, frame_rate: 24.0, render_ns: self.now, sender_instance: 1 };
        self.writer.write_frame(&meta, pixels).unwrap();
        self.frame += 1.0;
        for _ in 0..50 {
            self.now += MS;
            self.poll();
        }
    }

    fn counts(&self) -> WaveformCounts {
        self.waveform.counts()
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

const DWG_REC709: &str = "timeline=DaVinci WG/Intermediate\noutput=Rec.709 Gamma 2.4\n";
const ACESCCT_REC709: &str = "timeline=ACEScct\noutput=Rec.709 BT.1886\naces_version=2.0\n";

/// 10-bit code of a luma at video levels (black 64, white 940: #18), before the waveform rounds it.
fn code(v: f64) -> f64 {
    v * 876.0 + 64.0
}

/// Plays the chart in `space` through the Output Conversion of `setup` (the f64 `reference` of the accuracy suite)
/// and checks that every pixel of every grey patch is traced at the code the reference expects, give or take one
/// code of the waveform's rounding: the Output Conversion is exact to 1e-4 (#10), about a tenth of a code.
fn assert_grey_patches_trace_where_the_reference_says(name: &str, space: TimelineSpace, setup: &str, reference: Pipeline) {
    let (width, height) = (600u32, 400u32);
    let chart = ColorChecker::new(space);
    let frame = chart.frame(width, height);
    let mut b = Bench::new(name, setup);
    b.play(width, height, &frame.pixels);
    let counts = b.counts();
    let weights = luma_weights(&REC709);
    for patch in &chart.patches()[18..24] {
        let out = reference.apply(patch.encoded);
        let want = code(weights[0] * out[0] + weights[1] * out[1] + weights[2] * out[2]);
        let r = frame.patch_rect(patch.number);
        let mut off = Vec::new();
        for x in r.x0..r.x1 {
            let near: u32 = (0..1024u32).filter(|c| (*c as f64 - want).abs() <= 1.0).map(|c| counts.at(x, c)).sum();
            if near != r.y1 - r.y0 {
                off.push(x);
            }
        }
        assert!(off.is_empty(), "{space:?} patch {} {}: columns {off:?} are not all at code {want:.1} (within 1)", patch.number, patch.name);
    }
}

#[test]
fn a_davinci_intermediate_chart_traces_each_grey_patch_at_its_rec709_gamma_24_code() {
    assert_grey_patches_trace_where_the_reference_says("di", TimelineSpace::DavinciIntermediate, DWG_REC709, Pipeline::dwg_rec709());
}

#[test]
fn an_acescct_chart_traces_each_grey_patch_at_its_aces_2_rec709_code() {
    assert_grey_patches_trace_where_the_reference_says("acescct", TimelineSpace::AcesCct, ACESCCT_REC709, Pipeline::Aces2(Box::new(AcesOutputTransform::rec709_bt1886())));
}
