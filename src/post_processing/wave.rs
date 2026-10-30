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

const WAVE_SHADER_HANDLE: Handle<Shader> = crate::util::weak_from_u64(1792660281364049744);

/// Wave parameters.
///
/// Note that the parameters for the X axis causes a wave effect
/// towards the left- and right sides of the screen.
/// For example, if we have 1 wave in the X axis,
/// we will have one part of the screen stretched towards the right
/// horizontally, and one part stretched towards the left.
#[derive(Default, Debug, Copy, Clone, Component, ShaderType)]
pub struct Wave {
    /// How many waves in the x axis.
    pub waves_x: f32,

    /// How many waves in the y axis.
    pub waves_y: f32,

    /// How fast the x axis waves oscillate.
    pub speed_x: f32,

    /// How fast the y axis waves oscillate.
    pub speed_y: f32,

    /// How much displacement the x axis waves cause.
    pub amplitude_x: f32,

    /// How much displacement the y axis waves cause.
    pub amplitude_y: f32,
}

impl FromWorld for EffectPipeline<Wave> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "Wave",
            &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, uniform_buffer::<Wave>(true)),
            WAVE_SHADER_HANDLE,
        )
    }
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            WAVE_SHADER_HANDLE,
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/", "wave.wgsl"),
            Shader::from_wgsl
        );

        super::add_uniform_effect::<Wave, Wave>(app);
    }
}

impl SyncComponent for Wave {
    type Target = Self;
}

impl ExtractComponent for Wave {
    type QueryData = (&'static Self, &'static Camera);
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(
        (settings, camera): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        if !camera.is_active {
            return None;
        }

        Some(*settings)
    }
}
