//! Output Conversion: turning the Sender's Timeline Colour Space frames into the Output Colour Space the client
//! sees, on the GPU, so the Picture View shows and the Scopes measure the delivered picture (ADR 0001).
//!
//! The method is published maths (Stage 0, #10). Which conversion a Colour Setup gets, and how far it can be
//! trusted, is decided by [`Plan::for_setup`]: **exact** only for the Colour Setups the Stage 0 accuracy table
//! proved against Resolve's own renders; **approximated** (with the reason) where published maths is applied but
//! not proven; otherwise no conversion at all, and the label says why. The label never claims more than is proven.
//!
//! The transform definitions are not written twice. The matrices, constants and ACES 2.0 tables come from the
//! accuracy crate's f64 reference (`scopecreep_accuracy`, each module citing its source), the very code the
//! accuracy suite proves against Resolve; the shader only repeats the per-pixel steps, in 32-bit float. Think of
//! the accuracy crate as the recipe book and the shader as the cook: the cook gets the measured ingredients
//! handed over, and the tests taste every dish against the book (app/tests/output_conversion_seam.rs).
use crate::colour_setup::{ColourSetup, KnownColourSetup, OutputSpace, Science, TimelineSpace};
use crate::gpu_frame::{GpuFrame, new_texture_id};
use crate::measurement::{LumaWeights, MeasuredSpace, Measurement};
use scopecreep_accuracy::aces2::{self, AcesOutputTransform, Eotf};
use scopecreep_accuracy::colour::{M3, REC2020, mmul};
use scopecreep_accuracy::pipeline::{Encoding, Pipeline};
use scopecreep_accuracy::transfer;

/// The output side of a DaVinci Wide Gamut / Intermediate project (Colour Managed, no tone or gamut mapping).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DwgOutput {
    Rec709Gamma24,
    /// Resolve's "P3-D65": gamma 2.6.
    P3D65,
    /// Resolve's "Rec.2100 ST2084": 1.0 in DaVinci scene linear is 100 nit.
    Rec2100Pq,
    Rec2020Gamma24,
}

/// An ACES 2.0 output transform, by Resolve's (and the reference CTL's) name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcesOutput {
    Rec709Bt1886,
    P3D65,
    Rec2100Pq1000,
}

/// The maths applied to every pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// Resolve converts nothing: the Feed already is the Output Colour Space.
    Unchanged,
    /// DaVinci Intermediate decode, DWG to display primaries, display encoding.
    FromDwg(DwgOutput),
    /// ACEScct decode, then the ACES 2.0 Output Transform.
    FromAcescct(AcesOutput),
}

impl Method {
    /// The f64 published maths this method runs, as proven against Resolve in the accuracy crate.
    pub fn reference(self) -> Pipeline {
        match self {
            Method::Unchanged => Pipeline::Identity,
            Method::FromDwg(DwgOutput::Rec709Gamma24) => Pipeline::dwg_rec709(),
            Method::FromDwg(DwgOutput::P3D65) => Pipeline::dwg_p3d65(),
            Method::FromDwg(DwgOutput::Rec2100Pq) => Pipeline::dwg_rec2100_pq(),
            Method::FromDwg(DwgOutput::Rec2020Gamma24) => {
                Pipeline::dwg_to_display(REC2020, Encoding::SignedGamma(2.4))
            }
            Method::FromAcescct(AcesOutput::Rec709Bt1886) => {
                Pipeline::Aces2(Box::new(AcesOutputTransform::rec709_bt1886()))
            }
            Method::FromAcescct(AcesOutput::P3D65) => {
                Pipeline::Aces2(Box::new(AcesOutputTransform::p3_d65()))
            }
            Method::FromAcescct(AcesOutput::Rec2100Pq1000) => {
                Pipeline::Aces2(Box::new(AcesOutputTransform::rec2100_pq_1000()))
            }
        }
    }
}

/// How far a conversion can be trusted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Accuracy {
    /// Proven against Resolve's own renders (Stage 0 accuracy table, re-checked by the accuracy suite).
    Exact,
    /// Exact for ACES 2.0, which is assumed: Resolve does not tell scripts the ACES version.
    ExactIfAces2,
    /// Published maths, not proven to match Resolve for this Colour Setup; the reason says why.
    Approximated(String),
}

/// What the app does with the Feed for a Colour Setup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// The Colour Setup is not known: the Feed is shown and measured as sent.
    Unknown,
    Convert {
        method: Method,
        accuracy: Accuracy,
    },
    /// No conversion is available for this Colour Setup: the Feed is shown and measured as sent. Says why.
    Unsupported(String),
}

impl Plan {
    /// The conversion for a Colour Setup, labelled no better than Stage 0 proved (docs/stage0-report.md, section 1).
    pub fn for_setup(setup: Option<&ColourSetup>) -> Plan {
        let Some(s) = setup else { return Plan::Unknown };
        let not_measured =
            |what: &str| Accuracy::Approximated(format!("{what} not yet measured against Resolve"));
        if s.is_aces() {
            if s.timeline_space != TimelineSpace::Acescct {
                return Plan::Unsupported(format!("{} timelines not yet supported", s.timeline));
            }
            let version = s.effective_aces_version().unwrap_or_default();
            if version != "2.0" {
                return Plan::Unsupported(format!("ACES {version} not yet supported"));
            }
            let output = match &s.output_space {
                OutputSpace::Rec709Bt1886 => AcesOutput::Rec709Bt1886,
                OutputSpace::P3D65 => AcesOutput::P3D65,
                OutputSpace::Rec2100Pq1000 => AcesOutput::Rec2100Pq1000,
                _ => {
                    let other = &s.output;
                    return Plan::Unsupported(format!(
                        "ACES output transform {other} not yet supported"
                    ));
                }
            };
            let accuracy = if s.aces_version.is_some() {
                Accuracy::Exact
            } else {
                Accuracy::ExactIfAces2
            };
            return Plan::Convert {
                method: Method::FromAcescct(output),
                accuracy,
            };
        }
        match &s.science {
            Some(Science::Other(mode)) => {
                return Plan::Unsupported(format!("colour science {mode} not supported"));
            }
            Some(Science::Yrgb) => {
                // DaVinci YRGB converts nothing: the Colour Spaces are tags only.
                let accuracy = if s.timeline_space == TimelineSpace::Rec709Gamma24
                    && s.output_space == OutputSpace::Rec709Gamma24
                {
                    Accuracy::Exact
                } else {
                    not_measured("YRGB with these colour spaces")
                };
                return Plan::Convert {
                    method: Method::Unchanged,
                    accuracy,
                };
            }
            _ => {}
        }
        if s.timeline_space != TimelineSpace::DaVinciWideGamut {
            if s.timeline == s.output {
                return Plan::Convert {
                    method: Method::Unchanged,
                    accuracy: not_measured(&format!("{} to itself", s.timeline)),
                };
            }
            return Plan::Unsupported(format!("{} timelines not yet supported", s.timeline));
        }
        // A Colour Setup set by hand on the Sender node says nothing about the colour science, tone mapping or gamut
        // mapping (`science` is `None`). The conversion is proven only with mapping off, so it is not labelled exact.
        let by_hand = || Accuracy::Approximated("assumes tone and gamut mapping off".to_string());
        let proven = if s.science.is_none() {
            by_hand()
        } else {
            Accuracy::Exact
        };
        let (output, mut accuracy) = match &s.output_space {
            OutputSpace::Rec709Gamma24 => (DwgOutput::Rec709Gamma24, proven),
            OutputSpace::P3D65 => (DwgOutput::P3D65, proven),
            OutputSpace::Rec2100Pq => (DwgOutput::Rec2100Pq, proven),
            OutputSpace::Rec2020Gamma24 => (
                DwgOutput::Rec2020Gamma24,
                not_measured("Rec.2020 Gamma 2.4"),
            ),
            _ => return Plan::Unsupported(format!("output {} not yet supported", s.output)),
        };
        let mut unpublished = vec![];
        if let Some(t) = &s.tone_mapping {
            unpublished.push(format!("{t} tone mapping"));
        }
        if let Some(g) = &s.gamut_mapping {
            unpublished.push(format!("{g} gamut mapping"));
        }
        if !unpublished.is_empty() {
            accuracy = Accuracy::Approximated(format!(
                "{} not applied (unpublished)",
                unpublished.join(" and ")
            ));
        }
        Plan::Convert {
            method: Method::FromDwg(output),
            accuracy,
        }
    }

    /// The method to run on the GPU, `None` when the Feed is shown as sent.
    pub fn method(&self) -> Option<Method> {
        match self {
            Plan::Convert { method, .. } => Some(*method),
            _ => None,
        }
    }

    /// The Measured Space of the picture this plan shows: the Output Colour Space once converted, else not known
    /// (the Feed as sent is in the Timeline Colour Space).
    pub fn measured_space(&self, setup: Option<&ColourSetup>) -> Option<MeasuredSpace> {
        match self {
            Plan::Convert { .. } => setup.and_then(ColourSetup::measured_space),
            _ => None,
        }
    }

    /// The part of the Picture View's label that says what was done to the picture.
    pub fn label(&self) -> String {
        match self {
            Plan::Unknown => "as sent, no Output Conversion".to_string(),
            Plan::Unsupported(why) => format!("as sent, no Output Conversion: {why}"),
            Plan::Convert {
                accuracy: Accuracy::Exact,
                ..
            } => "Output Conversion exact".to_string(),
            Plan::Convert {
                accuracy: Accuracy::ExactIfAces2,
                ..
            } => "Output Conversion exact for ACES 2.0".to_string(),
            Plan::Convert {
                accuracy: Accuracy::Approximated(why),
                ..
            } => format!("Output Conversion approximated: {why}"),
        }
    }
}

/// What the waveform measures in for the picture the app shows: the Colour Setup's Output Colour Space and Levels
/// once the Feed is converted (or needs no conversion); when it is shown as sent, the space is not known and the
/// waveform says Rec.709 is assumed.
pub fn measurement(known: Option<KnownColourSetup>, luma: LumaWeights) -> Measurement {
    let setup = known.as_ref().map(|c| c.setup.clone());
    let mut m = Measurement::for_colour_setup(known, luma);
    if Plan::for_setup(setup.as_ref())
        .measured_space(setup.as_ref())
        .is_none()
    {
        m.space = MeasuredSpace::Rec709;
        m.assumed = true;
    }
    m
}

// ---------------------------------------------------------------------------------------------------- GPU

/// Floats in the ACES 2.0 tables: reach M, hues, cusp J, cusp M, upper hull gamma, each `TOTAL_TABLE_SIZE` long.
const TABLE_FLOATS: usize = 5 * aces2::TOTAL_TABLE_SIZE;
/// 32-bit words in the parameter block (see `Params` in the shader).
const PARAM_WORDS: usize = 4 + 4 + 5 * 24 + 6 * 4;

const SHADER_BODY: &str = include_str!("output_conversion.wgsl");

/// The shader, with the published constants written in from the accuracy crate.
fn shader_source() -> String {
    let consts: [(&str, f64); 25] = [
        ("DI_A", transfer::DI_A),
        ("DI_B", transfer::DI_B),
        ("DI_C", transfer::DI_C),
        ("DI_M", transfer::DI_M),
        ("DI_LOG_CUT", transfer::DI_LOG_CUT),
        ("CCT_Y_BRK", transfer::CCT_Y_BRK),
        ("CCT_A", transfer::CCT_A),
        ("CCT_B", transfer::CCT_B),
        ("PQ_M1", transfer::PQ_M1),
        ("PQ_M2", transfer::PQ_M2),
        ("PQ_C1", transfer::PQ_C1),
        ("PQ_C2", transfer::PQ_C2),
        ("PQ_C3", transfer::PQ_C3),
        ("TABLE_SIZE", aces2::TABLE_SIZE as f64),
        ("TOTAL_TABLE_SIZE", aces2::TOTAL_TABLE_SIZE as f64),
        ("HUE_LIMIT", aces2::HUE_LIMIT),
        ("REF_LUMINANCE", aces2::REF_LUMINANCE),
        ("J_SCALE", aces2::J_SCALE),
        ("CAM_NL_OFFSET", aces2::CAM_NL_OFFSET),
        ("SMOOTH_CUSPS", aces2::SMOOTH_CUSPS),
        ("CUSP_MID_BLEND", aces2::CUSP_MID_BLEND),
        ("FOCUS_GAIN_BLEND", aces2::FOCUS_GAIN_BLEND),
        ("COMPRESSION_THRESHOLD", aces2::COMPRESSION_THRESHOLD),
        ("CCT_LOG_SCALE", transfer::CCT_LOG_SCALE),
        ("CCT_LOG_OFFSET", transfer::CCT_LOG_OFFSET),
    ];
    let mut s = String::from(
        "// Published constants, from the accuracy crate (scopecreep_accuracy::{transfer, aces2}). NAME_LO is what\n\
         // 32-bit float loses of NAME, for the steps that need it back.\n",
    );
    for (name, value) in consts {
        let hi = value as f32;
        s += &format!(
            "const {name}: f32 = {hi:?};\nconst {name}_LO: f32 = {:?};\n",
            (value - hi as f64) as f32
        );
    }
    s += &format!("const BASE_INDEX: i32 = {};\n", aces2::BASE_INDEX);
    s + SHADER_BODY
}

/// A matrix as three rows of 32-bit floats, then three rows of what those floats lose (the shader adds it back).
fn push_matrix(words: &mut Vec<u32>, m: &M3) {
    for row in m {
        words.extend(row.iter().map(|&v| (v as f32).to_bits()));
        words.push(0);
    }
    for row in m {
        words.extend(
            row.iter()
                .map(|&v| ((v - (v as f32) as f64) as f32).to_bits()),
        );
        words.push(0);
    }
}

fn push4(words: &mut Vec<u32>, v: [f64; 4]) {
    words.extend(v.iter().map(|&x| (x as f32).to_bits()));
}

/// The shader's parameter block and tables for `pipeline` (the accuracy crate's f64 definition).
fn gpu_parameters(pipeline: &Pipeline, width: u32, height: u32) -> (Vec<u32>, Vec<f32>) {
    let zero: M3 = [[0.0; 3]; 3];
    let mut words = Vec::with_capacity(PARAM_WORDS);
    let mut tables = vec![0f32; TABLE_FLOATS];
    match pipeline {
        Pipeline::Identity => {
            words.extend([width, height, 0, 0]);
            push4(&mut words, [1.0, 0.0, 0.0, 0.0]);
            for _ in 0..5 {
                push_matrix(&mut words, &zero);
            }
            for _ in 0..6 {
                push4(&mut words, [0.0; 4]);
            }
        }
        Pipeline::DwgToDisplay {
            matrix, encoding, ..
        } => {
            let (code, enc) = match *encoding {
                Encoding::SignedGamma(g) => (0, [1.0 / g, 0.0, 0.0, 0.0]),
                Encoding::Pq { nits_per_unit } => (1, [0.0, nits_per_unit, 0.0, 0.0]),
            };
            words.extend([width, height, 1, code]);
            push4(&mut words, enc);
            push_matrix(&mut words, matrix);
            for _ in 0..4 {
                push_matrix(&mut words, &zero);
            }
            for _ in 0..6 {
                push4(&mut words, [0.0; 4]);
            }
        }
        Pipeline::Aces2(ot) => {
            let p = &ot.odt;
            let (code, enc) = match ot.eotf {
                Eotf::St2084 => (
                    1,
                    [
                        0.0,
                        aces2::REF_LUMINANCE,
                        ot.peak_luminance / aces2::REF_LUMINANCE,
                        0.0,
                    ],
                ),
                Eotf::Gamma26 => (
                    2,
                    [
                        1.0 / 2.6,
                        0.0,
                        ot.peak_luminance / aces2::REF_LUMINANCE,
                        0.0,
                    ],
                ),
                Eotf::Bt1886Gamma24 => (
                    2,
                    [
                        1.0 / 2.4,
                        0.0,
                        ot.peak_luminance / aces2::REF_LUMINANCE,
                        0.0,
                    ],
                ),
            };
            words.extend([width, height, 2, code]);
            push4(&mut words, enc);
            // ACEScct decodes to AP1 linear. The reference goes AP1 -> AP0 -> AP1, clamps, then AP1 -> AP0 -> CAM16;
            // the shader clamps in AP1 directly and folds the last two matrices into one.
            push_matrix(
                &mut words,
                &mmul(&ot.ap1_to_ap0, &p.input_params.m_rgb_to_cam16_c),
            );
            push_matrix(&mut words, &p.input_params.m_cone_response_to_aab);
            push_matrix(&mut words, &p.limit_params.m_aab_to_cone_response);
            push_matrix(&mut words, &p.limit_params.m_cam16_c_to_rgb);
            push_matrix(&mut words, &ot.matrix_limit_to_display);
            let ip = &p.input_params;
            push4(&mut words, [p.ts.n_r, p.ts.g, p.ts.t_1, p.ts.s_2]);
            push4(&mut words, [p.ts.m_2, p.ts.forward_limit, ip.f_l_n, 0.0]);
            push4(&mut words, [ip.cz, ip.inv_cz, ip.a_w_j, ip.inv_a_w_j]);
            push4(
                &mut words,
                [p.limit_j_max, p.model_gamma_inv, p.sat, p.sat_thr],
            );
            push4(
                &mut words,
                [p.compr, p.chroma_compress_scale, p.mid_j, p.focus_dist],
            );
            push4(
                &mut words,
                [
                    p.lower_hull_gamma_inv,
                    p.hue_linearity_search_range[0] as f64,
                    p.hue_linearity_search_range[1] as f64,
                    0.0,
                ],
            );
            let n = aces2::TOTAL_TABLE_SIZE;
            for i in 0..n {
                tables[i] = p.table_reach_m[i] as f32;
                tables[n + i] = p.table_hues[i] as f32;
                tables[2 * n + i] = p.table_gamut_cusps[i][0] as f32;
                tables[3 * n + i] = p.table_gamut_cusps[i][1] as f32;
                tables[4 * n + i] = p.table_upper_hull_gamma[i] as f32;
            }
        }
    }
    debug_assert_eq!(words.len(), PARAM_WORDS);
    (words, tables)
}

/// The Output Conversion's GPU state. Lives on the Feed thread.
pub struct OutputConversion {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    tables: wgpu::Buffer,
    plan: Plan,
    /// The method and picture size whose parameters are in `params` and `tables`.
    loaded: Option<(Method, u32, u32)>,
    /// The converted picture.
    out: Option<GpuFrame>,
    /// Bound to (source texture, converted texture).
    bound: Option<(u64, u64, wgpu::BindGroup)>,
    /// Reference pipelines already built (the ACES 2.0 tables take a moment to build).
    built: Vec<(Method, Pipeline)>,
}

impl OutputConversion {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> OutputConversion {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("output conversion"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("output conversion"),
            layout: None,
            module: &shader,
            entry_point: Some("convert"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output conversion params"),
            size: (PARAM_WORDS * 4) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tables = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("output conversion tables"),
            size: (TABLE_FLOATS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        OutputConversion {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            params,
            tables,
            plan: Plan::Unknown,
            loaded: None,
            out: None,
            bound: None,
            built: Vec::new(),
        }
    }

    /// Follow the Colour Setup; call after every Feed poll with the frame on screen. When the conversion changes
    /// (the Colour Setup arrived or changed, possibly with Resolve stopped and no new frame coming), the frame on
    /// screen is converted again and `then` records what reads it (the Scopes, the Picture View) into the same
    /// submission. Returns whether that happened.
    pub fn follow(
        &mut self,
        setup: Option<&ColourSetup>,
        on_screen: Option<&GpuFrame>,
        then: impl FnOnce(&mut wgpu::CommandEncoder, &GpuFrame),
    ) -> bool {
        let plan = Plan::for_setup(setup);
        if plan == self.plan {
            return false;
        }
        self.plan = plan;
        self.again(on_screen, then);
        true
    }

    /// Convert the frame on screen again and let `then` record what reads it. For when something other than the
    /// frame changed what the displays measure (a setting), with Resolve stopped and no new frame coming.
    pub fn again(
        &mut self,
        on_screen: Option<&GpuFrame>,
        then: impl FnOnce(&mut wgpu::CommandEncoder, &GpuFrame),
    ) {
        if let Some(frame) = on_screen {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("output conversion again"),
                });
            let shown = self.convert(&mut encoder, frame);
            then(&mut encoder, shown);
            self.queue.submit([encoder.finish()]);
        }
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Whether the picture shown is a converted one (else it is the Feed frame itself).
    fn converts(&self) -> bool {
        self.plan.method().is_some_and(|m| m != Method::Unchanged)
    }

    /// Record the conversion of `frame` into `encoder` and return the picture to show and measure: the converted
    /// one, or `frame` itself when there is nothing to convert.
    pub fn convert<'a>(
        &'a mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &'a GpuFrame,
    ) -> &'a GpuFrame {
        let Some(method) = self.plan.method().filter(|m| *m != Method::Unchanged) else {
            return frame;
        };
        let (width, height) = (frame.width, frame.height);
        if self
            .out
            .as_ref()
            .is_none_or(|o| o.width != width || o.height != height)
        {
            self.out = Some(GpuFrame {
                texture: self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("converted frame"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba32Float,
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                }),
                width,
                height,
                bottom_up: frame.bottom_up,
                texture_id: new_texture_id(),
            });
        }
        if self.loaded != Some((method, width, height)) {
            if !self.built.iter().any(|(m, _)| *m == method) {
                self.built.push((method, method.reference()));
            }
            let reference = &self.built.iter().find(|(m, _)| *m == method).unwrap().1;
            let (words, tables) = gpu_parameters(reference, width, height);
            self.queue
                .write_buffer(&self.params, 0, bytemuck::cast_slice(&words));
            self.queue
                .write_buffer(&self.tables, 0, bytemuck::cast_slice(&tables));
            self.loaded = Some((method, width, height));
        }
        let out = self.out.as_mut().unwrap();
        out.bottom_up = frame.bottom_up;
        if self
            .bound
            .as_ref()
            .is_none_or(|(src, dst, _)| *src != frame.texture_id || *dst != out.texture_id)
        {
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("output conversion"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &frame.texture.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(
                            &out.texture.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.tables.as_entire_binding(),
                    },
                ],
            });
            self.bound = Some((frame.texture_id, out.texture_id, bind_group));
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("output conversion"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bound.as_ref().unwrap().2, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        self.out.as_ref().unwrap()
    }

    /// The picture shown for `frame` (converted or as sent), read back from the GPU, top row first, RGB. Waits for
    /// the GPU: for tests and the accuracy suite, not per frame.
    pub fn read_shown(&self, frame: &GpuFrame) -> Vec<[f32; 3]> {
        let shown = if self.converts() {
            self.out.as_ref().unwrap_or(frame)
        } else {
            frame
        };
        read_frame(&self.device, &self.queue, shown)
    }
}

/// A frame on the GPU read back, top row first, RGB.
pub fn read_frame(device: &wgpu::Device, queue: &wgpu::Queue, frame: &GpuFrame) -> Vec<[f32; 3]> {
    let (width, height) = (frame.width as usize, frame.height as usize);
    let row_bytes = width * 16;
    let padded_row = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame readback"),
        size: (padded_row * height) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &frame.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row as u32),
                rows_per_image: Some(frame.height),
            },
        },
        wgpu::Extent3d {
            width: frame.width,
            height: frame.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    readback.map_async(wgpu::MapMode::Read, .., |_| {});
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the GPU finishes");
    let data = readback.get_mapped_range(..).expect("mapped");
    let mut out = Vec::with_capacity(width * height);
    for row in 0..height {
        let source_row = if frame.bottom_up {
            height - 1 - row
        } else {
            row
        };
        let floats: &[f32] = bytemuck::cast_slice(
            &data[source_row * padded_row..source_row * padded_row + row_bytes],
        );
        out.extend(floats.as_chunks::<4>().0.iter().map(|p| [p[0], p[1], p[2]]));
    }
    out
}
