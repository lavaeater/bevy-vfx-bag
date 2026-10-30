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

pub(crate) const FLIP_SHADER_HANDLE: Handle<Shader> =
    crate::util::weak_from_u64(1649866799156783187);

impl FromWorld for EffectPipeline<Flip> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "Flip",
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                uniform_buffer::<FlipUniform>(true),
            ),
            FLIP_SHADER_HANDLE,
        )
    }
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            FLIP_SHADER_HANDLE,
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/", "flip.wgsl"),
            Shader::from_wgsl
        );

        super::add_uniform_effect::<Flip, FlipUniform>(app);
    }
}

#[doc(hidden)]
/// The uniform representation of [`Flip`].
#[derive(Debug, ShaderType, Clone, Component)]
pub struct FlipUniform {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

impl From<Flip> for FlipUniform {
    fn from(flip: Flip) -> Self {
        let uv = match flip {
            Flip::None => [0.0, 0.0],
            Flip::Horizontal => [1.0, 0.0],
            Flip::Vertical => [0.0, 1.0],
            Flip::HorizontalVertical => [1.0, 1.0],
        };

        Self { x: uv[0], y: uv[1] }
    }
}

/// Which way to flip the texture.
#[derive(Debug, Default, Copy, Clone, Component)]
pub enum Flip {
    /// Don't flip.
    None,

    /// Flip horizontally.
    #[default]
    Horizontal,

    /// Flip vertically.
    Vertical,

    /// Flip both axes.
    HorizontalVertical,
}

impl Display for Flip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl SyncComponent for Flip {
    type Target = FlipUniform;
}

impl ExtractComponent for Flip {
    type QueryData = (&'static Self, &'static Camera);
    type QueryFilter = ();
    type Out = FlipUniform;

    fn extract_component(
        (settings, camera): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        if !camera.is_active {
            return None;
        }

        Some((*settings).into())
    }
}
