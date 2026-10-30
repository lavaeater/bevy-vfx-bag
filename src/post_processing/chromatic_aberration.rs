use std::{f32::consts::PI, fmt::Display};

use bevy::{
    asset::load_internal_asset,
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::ExtractComponent,
        render_resource::{
            binding_types::uniform_buffer, BindGroupLayoutEntries, ShaderStages, ShaderType,
        },
        sync_component::SyncComponent,
    },
    shader::Shader,
};

use super::EffectPipeline;

pub(crate) const CHROMATIC_ABERRATION_SHADER_HANDLE: Handle<Shader> =
    crate::util::weak_from_u64(4357337502039082134);

impl FromWorld for EffectPipeline<ChromaticAberration> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "ChromaticAberration",
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                uniform_buffer::<ChromaticAberration>(true),
            ),
            CHROMATIC_ABERRATION_SHADER_HANDLE,
        )
    }
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CHROMATIC_ABERRATION_SHADER_HANDLE,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/shaders/",
                "chromatic-aberration.wgsl"
            ),
            Shader::from_wgsl
        );

        super::add_uniform_effect::<ChromaticAberration, ChromaticAberration>(app);
    }
}

/// Chromatic Aberration settings.
#[derive(Debug, Copy, Clone, Component, ShaderType)]
pub struct ChromaticAberration {
    /// The direction (in UV space) the red channel is offset in.
    /// Will be normalized.
    pub dir_r: Vec2,

    /// How far (in UV space) the red channel should be displaced.
    pub magnitude_r: f32,

    /// The direction (in UV space) the green channel is offset in.
    /// Will be normalized.
    pub dir_g: Vec2,

    /// How far (in UV space) the green channel should be displaced.
    pub magnitude_g: f32,

    /// The direction (in UV space) the blue channel is offset in.
    /// Will be normalized.
    pub dir_b: Vec2,

    /// How far (in UV space) the blue channel should be displaced.
    pub magnitude_b: f32,
}

impl ChromaticAberration {
    /// Adds the given diff to the magnitude of all color channels.
    pub fn add_magnitude(&mut self, diff: f32) {
        self.magnitude_r += diff;
        self.magnitude_g += diff;
        self.magnitude_b += diff;
    }
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        let one_third = (2. / 3.) * PI;

        Self {
            dir_r: Vec2::from_angle(0. * one_third),
            magnitude_r: 0.01,
            dir_g: Vec2::from_angle(1. * one_third),
            magnitude_g: 0.01,
            dir_b: Vec2::from_angle(2. * one_third),
            magnitude_b: 0.01,
        }
    }
}

impl Display for ChromaticAberration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let base_angle = Vec2::new(1., 0.);
        let angle = |color_dir| base_angle.angle_to(color_dir) * 180. / PI + 180.;

        write!(
            f,
            "Chromatic Aberration [magnitude, angle]:  R: [{:.3}, {:4.0}°] G: [{:.3}, {:4.0}°] B: [{:.3}, {:4.0}°]",
            self.magnitude_r,
            angle(self.dir_r),
            self.magnitude_g,
            angle(self.dir_g),
            self.magnitude_b,
            angle(self.dir_b)
        )
    }
}

impl SyncComponent for ChromaticAberration {
    type Target = ChromaticAberration;
}

impl ExtractComponent for ChromaticAberration {
    type QueryData = (&'static Self, &'static Camera);
    type QueryFilter = ();
    type Out = ChromaticAberration;

    fn extract_component(
        (settings, camera): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        if !camera.is_active {
            return None;
        }

        Some(*settings)
    }
}
