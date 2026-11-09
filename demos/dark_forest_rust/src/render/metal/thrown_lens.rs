//! `thrown_lens` draws the players' thrown wormhole mouths' views of the static world for the lens
//! pass, on the GPU and a few faces a frame: when a mouth appears, the static tiles within
//! `THROWN_LENS_RANGE_M` of it, and the sky, are drawn into its slot's row of six cube faces
//! (oriented as `LENS_FACES`, `THROWN_LENS_FACE_PX` each) in HDR colour and depth,
//! `THROWN_FACES_PER_FRAME` faces a frame, instead of the start-up capture's read-back (which would
//! stall a frame). The lens pass shows a thrown pair once both its mouths' faces are drawn.
//!
//! Example:
//!     let jobs = thrown.plan_faces(frame.thrown_lens_mouths);
//!     self.encode_thrown_faces(&command_buffer, frame, settings, targets, &cascades, &buffers, &jobs);
//!
//! Interface: `ThrownLens`, `ThrownFaceJob`, `MetalRenderer::encode_thrown_faces`,
//! `MetalRenderer::get_frame_lens`.

use super::*;
use crate::render::{MAX_THROWN_LENS_MOUTH_COUNT, THROWN_LENS_FACE_PX, ThrownLensMouth};

/// `THROWN_FACES_PER_FRAME` is how many faces of thrown mouths' views a frame draws.
const THROWN_FACES_PER_FRAME: usize = 2;
/// `THROWN_LENS_RANGE_M` is how far from a thrown mouth the static tiles its view draws reach;
/// the forest's haze hides what is farther.
const THROWN_LENS_RANGE_M: f32 = 30.0;

/// `ThrownSlot` is one thrown mouth's view: the mouth's centre and how many of its faces are drawn.
#[derive(Clone, Copy)]
struct ThrownSlot {
    centre_m: Vec3,
    drawn_face_count: usize,
}

/// `ThrownFaceJob` is one face to draw this frame: its slot, its face (an index into
/// `LENS_FACES`) and the mouth's centre.
#[derive(Clone, Copy)]
pub(super) struct ThrownFaceJob {
    slot: usize,
    face: usize,
    centre_m: Vec3,
}

/// `ThrownLens` is where the thrown mouths' views are drawn: one array slice per slot, a row of six
/// faces, HDR colour and depth, and which mouth each slot holds.
pub(super) struct ThrownLens {
    pub(super) color: Texture,
    pub(super) depth: Texture,
    slots: [Option<ThrownSlot>; MAX_THROWN_LENS_MOUTH_COUNT],
    slot_of_mouth: Vec<usize>,
}

impl ThrownLens {
    /// `New` makes empty slots for `MAX_THROWN_LENS_MOUTH_COUNT` mouths on `device`.
    ///
    /// Example: `let thrown: ThrownLens = ThrownLens::New(&device);`
    #[allow(non_snake_case)]
    pub(super) fn New(device: &ProtocolObject<dyn MTLDevice>) -> ThrownLens {
        let slot_spec = |format: MTLPixelFormat| -> TextureSpec {
            TextureSpec {
                texture_type: MTLTextureType::Type2DArray,
                array_length: MAX_THROWN_LENS_MOUTH_COUNT,
                ..TextureSpec::render_target(
                    format,
                    THROWN_LENS_FACE_PX * LENS_FACES.len(),
                    THROWN_LENS_FACE_PX,
                )
            }
        };
        ThrownLens {
            color: create_texture(device, &slot_spec(HDR_FORMAT), "thrown lens color"),
            depth: create_texture(device, &slot_spec(DEPTH_FORMAT), "thrown lens depth"),
            slots: [None; MAX_THROWN_LENS_MOUTH_COUNT],
            slot_of_mouth: Vec::new(),
        }
    }

    /// `plan_faces` gives each of `mouths` a slot (the one it had while its centre stays, else a
    /// free one, freeing the slots of mouths gone) and lists the faces to draw this frame, the
    /// earliest slots' first; it counts them as drawn.
    ///
    /// Example: `let jobs: Vec<ThrownFaceJob> = thrown.plan_faces(frame.thrown_lens_mouths);`
    pub(super) fn plan_faces(&mut self, mouths: &[ThrownLensMouth]) -> Vec<ThrownFaceJob> {
        assert!(
            mouths.len() <= MAX_THROWN_LENS_MOUTH_COUNT,
            "the lens takes at most {MAX_THROWN_LENS_MOUTH_COUNT} thrown mouths, not {}",
            mouths.len()
        );
        for slot in &mut self.slots {
            if slot.is_some_and(|kept| !mouths.iter().any(|m| m.mouth.centre_m == kept.centre_m)) {
                *slot = None;
            }
        }
        self.slot_of_mouth = mouths
            .iter()
            .map(|thrown| {
                let centre_m: Vec3 = thrown.mouth.centre_m;
                if let Some(index) = self
                    .slots
                    .iter()
                    .position(|slot| slot.is_some_and(|kept| kept.centre_m == centre_m))
                {
                    return index;
                }
                let index: usize = self
                    .slots
                    .iter()
                    .position(Option::is_none)
                    .expect("a free slot for every thrown mouth");
                self.slots[index] = Some(ThrownSlot {
                    centre_m,
                    drawn_face_count: 0,
                });
                index
            })
            .collect();
        let mut jobs: Vec<ThrownFaceJob> = Vec::new();
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let Some(slot) = slot else {
                continue;
            };
            while slot.drawn_face_count < LENS_FACES.len() && jobs.len() < THROWN_FACES_PER_FRAME {
                jobs.push(ThrownFaceJob {
                    slot: index,
                    face: slot.drawn_face_count,
                    centre_m: slot.centre_m,
                });
                slot.drawn_face_count += 1;
            }
        }
        jobs
    }

    /// `get_drawn_slot` gives the slot of thrown mouth `index` (as last given to `plan_faces`) once
    /// all its faces are drawn.
    ///
    /// Example: `let slot: Option<usize> = thrown.get_drawn_slot(0);`
    fn get_drawn_slot(&self, index: usize) -> Option<usize> {
        let slot: usize = self.slot_of_mouth[index];
        self.slots[slot]
            .is_some_and(|kept| kept.drawn_face_count == LENS_FACES.len())
            .then_some(slot)
    }
}

impl MetalRenderer {
    /// `encode_thrown_faces` draws `jobs`' faces: from each mouth's centre, the static draws within
    /// `THROWN_LENS_RANGE_M` and the sky, lit as the frame is (every point in the last shadow
    /// cascade, as the live slots are), into its slot of `self.thrown_lens`. A slot's first face
    /// clears the slot.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_thrown_faces(
        &self,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        frame: &FrameDescription,
        settings: &RenderSettings,
        targets: &Targets,
        cascades: &Cascades,
        buffers: &FrameBuffers,
        jobs: &[ThrownFaceJob],
    ) {
        let Some(thrown) = &self.thrown_lens else {
            return;
        };
        let environment: &EnvironmentState = self.environment.as_ref().expect("no environment");
        let face_px: f32 = THROWN_LENS_FACE_PX as f32;
        let alpha_mode_of = |draw: &DrawItem| self.materials[draw.material.0 as usize].alpha_mode;
        for job in jobs {
            let load: MTLLoadAction = if job.face == 0 {
                MTLLoadAction::Clear
            } else {
                MTLLoadAction::Load
            };
            let descriptor: Retained<MTLRenderPassDescriptor> = color_pass(
                &thrown.color,
                load,
                Some((&thrown.depth, load, MTLStoreAction::Store)),
            );
            let color_attachment =
                unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
            color_attachment.setSlice(job.slot);
            descriptor.depthAttachment().setSlice(job.slot);
            let encoder = command_buffer
                .renderCommandEncoderWithDescriptor(&descriptor)
                .expect("render encoder creation failed");
            encoder.setFrontFacingWinding(MTLWinding::CounterClockwise);
            let (forward, up) = LENS_FACES[job.face];
            let camera: crate::render::Camera = crate::render::Camera {
                view: crate::render::camera_math::make_view_rh(job.centre_m, forward, up),
                position_m: job.centre_m,
                fov_y_rad: std::f32::consts::FRAC_PI_2,
                ..frame.camera
            };
            let proj: Mat4 = crate::render::camera_math::make_perspective_infinite_reverse_rh(
                std::f32::consts::FRAC_PI_2,
                1.0,
                camera.near_m,
            );
            let face_frame: FrameDescription = FrameDescription { camera, ..*frame };
            let mut face_uniforms: GpuFrameUniforms =
                self.frame_uniforms(&face_frame, settings, targets, cascades, proj, proj);
            face_uniforms.screen_size =
                Vec4::new(face_px, face_px, face_px.recip(), face_px.recip());
            face_uniforms.foveation = Vec4::new(face_px.recip(), face_px.recip(), 0.0, 0.0);
            face_uniforms.noise_offset_px = Vec4::ZERO;
            face_uniforms.render_params.z = 0.0;
            face_uniforms.cascade_split_far_m = Vec4::new(
                LENS_LIVE_UNUSED_SPLIT_M,
                LENS_LIVE_UNUSED_SPLIT_M,
                LENS_LIVE_UNUSED_SPLIT_M,
                LENS_LIVE_LAST_SPLIT_M,
            );
            let face_frustum: Frustum = Frustum::from_view_proj(face_uniforms.view_proj);
            let draws: Vec<&DrawItem> = frame.world_draws[..frame.static_draw_count]
                .iter()
                .filter(|draw| {
                    draw.bounds_center_m.distance(job.centre_m)
                        < THROWN_LENS_RANGE_M + draw.bounds_radius_m
                        && face_frustum.contains_sphere(draw.bounds_center_m, draw.bounds_radius_m)
                })
                .collect();
            encoder.setViewport(MTLViewport {
                originX: f64::from(face_px) * job.face as f64,
                originY: 0.0,
                width: f64::from(face_px),
                height: f64::from(face_px),
                znear: 0.0,
                zfar: 1.0,
            });
            let face_buffers: FrameBuffers = FrameBuffers {
                frame: &face_uniforms,
                ..*buffers
            };
            self.bind_scene_inputs(&encoder, &face_buffers, targets);
            let face_pass: GpuPassUniforms = GpuPassUniforms {
                view_proj: face_uniforms.view_proj,
                flags: Vec4::ZERO,
            };
            set_vertex_bytes(&encoder, as_bytes(&face_pass), 2);
            encoder.setCullMode(MTLCullMode::Back);
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_write));
            for (pipeline_pair, alpha_mode) in [
                (&self.pipelines.forward_opaque, AlphaMode::Opaque),
                (&self.pipelines.forward_masked, AlphaMode::Mask),
            ] {
                let mode_draws: Vec<&DrawItem> = draws
                    .iter()
                    .copied()
                    .filter(|draw| alpha_mode_of(draw) == alpha_mode)
                    .collect();
                self.draw_meshes(
                    &encoder,
                    pipeline_pair,
                    &mode_draws,
                    ScenePass::Forward,
                    &face_buffers,
                    false,
                );
            }
            encoder.setCullMode(MTLCullMode::None);
            encoder.setDepthStencilState(Some(&self.pipelines.depth_states.greater_equal_read));
            encoder.setRenderPipelineState(&self.pipelines.sky);
            unsafe {
                encoder.setFragmentTexture_atIndex(Some(&environment.ibl.environment), 0);
                encoder.drawPrimitives_vertexStart_vertexCount(
                    MTLPrimitiveType::Triangle,
                    0,
                    FULLSCREEN_VERTEX_COUNT,
                );
            }
            encoder.endEncoding();
        }
    }

    /// `get_frame_lens` gives the mouths the lens pass shows this frame and its uniforms: the
    /// map's (`static_uniforms`), then the thrown mouths of `thrown_mouths` whose views, and their
    /// partners', are drawn, each with its slot and tint, and `near_m`, the camera's near plane.
    ///
    /// Example: `let (mouths, uniforms) = self.get_frame_lens(&live.mouths, static_uniforms, frame.thrown_lens_mouths, near_m);`
    pub(super) fn get_frame_lens(
        &self,
        map_mouths: &[LensMouth],
        static_uniforms: &gpu_types::GpuLensUniforms,
        thrown_mouths: &[ThrownLensMouth],
        near_m: f32,
    ) -> (Vec<LensMouth>, gpu_types::GpuLensUniforms) {
        let mut mouths: Vec<LensMouth> = map_mouths.to_vec();
        let mut uniforms: gpu_types::GpuLensUniforms = *static_uniforms;
        uniforms.live_params.z = near_m;
        let Some(thrown) = &self.thrown_lens else {
            return (mouths, uniforms);
        };
        let shown: Vec<usize> = (0..thrown_mouths.len())
            .filter(|&index| {
                thrown.get_drawn_slot(index).is_some()
                    && thrown
                        .get_drawn_slot(thrown_mouths[index].mouth.partner)
                        .is_some()
            })
            .collect();
        let first: usize = map_mouths.len();
        for (k, &index) in shown.iter().enumerate() {
            let thrown_mouth: &ThrownLensMouth = &thrown_mouths[index];
            let partner: usize = first
                + shown
                    .iter()
                    .position(|&other| other == thrown_mouth.mouth.partner)
                    .expect("a shown thrown mouth's partner is shown");
            let mouth: LensMouth = LensMouth {
                partner,
                ..thrown_mouth.mouth
            };
            uniforms.centres[first + k] = mouth.centre_m.extend(mouth.lens_throat_m);
            uniforms.shapes[first + k] = Vec4::new(
                mouth.reach_m,
                mouth.lambda_squared,
                partner as f32,
                mouth.floor_m,
            );
            let slot: usize = thrown
                .get_drawn_slot(index)
                .expect("a shown mouth is drawn");
            uniforms.thrown_looks[k] = Vec4::new(
                slot as f32,
                thrown_mouth.tint_rgb.x,
                thrown_mouth.tint_rgb.y,
                thrown_mouth.tint_rgb.z,
            );
            mouths.push(mouth);
        }
        uniforms.params.x = mouths.len() as f32;
        (mouths, uniforms)
    }
}
