use std::fmt::Display;

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

pub(crate) const BLUR_SHADER_HANDLE: Handle<Shader> =
    crate::util::weak_from_u64(11044253213698850613);

impl FromWorld for EffectPipeline<Blur> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "Blur",
            &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, uniform_buffer::<Blur>(true)),
            BLUR_SHADER_HANDLE,
        )
    }
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            BLUR_SHADER_HANDLE,
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/", "blur.wgsl"),
            Shader::from_wgsl
        );

        super::add_uniform_effect::<Blur, Blur>(app);
    }
}

/// Blur settings.
#[derive(Debug, Copy, Clone, Component, ShaderType)]
pub struct Blur {
    /// How blurry the output image should be.
    /// If `0.0`, no blur is applied.
    /// `1.0` is "fully blurred", but higher values will produce interesting results.
    pub amount: f32,

    /// How far away the blur should sample points away from the origin point
    /// when blurring.
    /// This is in UV coordinates, so small (positive) values are expected (`0.01` is a good start).
    pub kernel_radius: f32,
}

impl Default for Blur {
    fn default() -> Self {
        Self {
            amount: 0.5,
            kernel_radius: 0.01,
        }
    }
}

impl Display for Blur {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Blur amount: {}, radius: {}",
            self.amount, self.kernel_radius
        )
    }
}

impl SyncComponent for Blur {
    type Target = Blur;
}

impl ExtractComponent for Blur {
    type QueryData = (&'static Self, &'static Camera);
    type QueryFilter = ();
    type Out = Blur;

    fn extract_component(
        (settings, camera): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        if !camera.is_active {
            return None;
        }

        Some(*settings)
    }
}
