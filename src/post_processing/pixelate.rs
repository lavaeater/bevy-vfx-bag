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

pub(crate) const PIXELATE_SHADER_HANDLE: Handle<Shader> =
    crate::util::weak_from_u64(11093977931118718560);

impl FromWorld for EffectPipeline<Pixelate> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "Pixelate",
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                uniform_buffer::<Pixelate>(true),
            ),
            PIXELATE_SHADER_HANDLE,
        )
    }
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            PIXELATE_SHADER_HANDLE,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/shaders/",
                "pixelate.wgsl"
            ),
            Shader::from_wgsl
        );

        super::add_uniform_effect::<Pixelate, Pixelate>(app);
    }
}

/// Pixelate settings.
#[derive(Debug, ShaderType, Component, Clone, Copy)]
pub struct Pixelate {
    /// How many pixels in the width and height in a block after pixelation. One block has a constant color within it.
    ///
    /// The shader sets a lower bound to 1.0, since that would not change the outcome.
    pub block_size: f32,
}

impl Default for Pixelate {
    fn default() -> Self {
        Self { block_size: 8.0 }
    }
}

impl Display for Pixelate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pixelate block size: {}", self.block_size)
    }
}

impl SyncComponent for Pixelate {
    type Target = Pixelate;
}

impl ExtractComponent for Pixelate {
    type QueryData = (&'static Self, &'static Camera);
    type QueryFilter = ();
    type Out = Pixelate;

    fn extract_component(
        (settings, camera): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        if !camera.is_active {
            return None;
        }

        Some(*settings)
    }
}
