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
use std::fmt::Display;

use super::{EffectPipeline, EffectShaderDef};

pub(crate) const MASK_SHADER_HANDLE: Handle<Shader> =
    crate::util::weak_from_u64(1059400090272595510);

impl FromWorld for EffectPipeline<Mask> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "Masks",
            &BindGroupLayoutEntries::single(
                ShaderStages::FRAGMENT,
                uniform_buffer::<MaskUniform>(true),
            ),
            MASK_SHADER_HANDLE,
        )
    }
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            MASK_SHADER_HANDLE,
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/", "masks.wgsl"),
            Shader::from_wgsl
        );

        // The pipeline is specialized on the variant via `EffectShaderDef<Mask>`.
        super::add_uniform_effect::<Mask, MaskUniform>(app);
    }
}

/// This controls the parameters of the effect.
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone, Component)]
pub enum MaskVariant {
    /// Rounded square type mask.
    ///
    /// One use of this mask is to post-process _other_ effects which might
    /// have artifacts around the edges.
    /// This mask can then attenuate that effect and thus remove the effects of the
    /// artifacts.
    ///
    /// Strength value guidelines for use in [`Mask`]:
    ///
    /// Low end:    3.0 almost loses the square shape.
    /// High end:   100.0 has almost sharp, thin edges.
    Square,

    /// Rounded square type mask, but more oval like a CRT television.
    ///
    /// This effect can be used as a part of a retry-style effect.
    ///
    /// Strength value guidelines for use in [`Mask`]:
    ///
    /// Low end:    3000.0 almost loses the CRT shape.
    /// High end:   500000.0 "inflates" the effect a bit.
    Crt,

    /// Vignette mask.
    ///
    /// This effect can be used to replicate the classic photography
    /// light attenuation seen at the edges of photos.
    ///
    /// Strength value guidelines for use in [`Mask`]:
    ///
    /// Low end:    0.10 gives a very subtle effect.
    /// High end:   1.50 is almost a spotlight in the middle of the screen.
    Vignette,
}

impl MaskVariant {
    /// The shader definition selecting this variant in the shader.
    fn shader_def(self) -> &'static str {
        match self {
            MaskVariant::Square => "SQUARE",
            MaskVariant::Crt => "CRT",
            MaskVariant::Vignette => "VIGNETTE",
        }
    }
}

/// A darkening mask on the outer edges of the image.
#[derive(Debug, Copy, Clone, Component)]
pub struct Mask {
    /// The strength parameter of the mask in use.
    ///
    /// See [`MaskVariant`] for guidelines on which range of values make sense
    /// for the variant in use.
    ///
    /// Run the masks example to experiment with these values interactively.
    pub strength: f32,

    /// How much the mask is faded: 1.0 - mask has no effect, 0.0 - mask is in full effect
    pub fade: f32,

    /// Which [`MaskVariant`] to produce.
    pub variant: MaskVariant,
}

impl Display for Mask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mask {:?}, strength: {} fade: {}",
            self.variant, self.strength, self.fade
        )
    }
}

impl Mask {
    /// Create a new square mask with a reasonable strength value.
    pub fn square() -> Self {
        Self {
            strength: 20.,
            fade: 0.,
            variant: MaskVariant::Square,
        }
    }

    /// Create a new CRT mask with a reasonable strength value.
    pub fn crt() -> Self {
        Self {
            strength: 80000.,
            fade: 0.,
            variant: MaskVariant::Crt,
        }
    }

    /// Create a new vignette mask with a reasonable strength value.
    pub fn vignette() -> Self {
        Self {
            strength: 0.66,
            fade: 0.,
            variant: MaskVariant::Vignette,
        }
    }
}

impl Default for Mask {
    fn default() -> Self {
        Self::vignette()
    }
}

#[doc(hidden)]
/// [`Mask`] as a uniform.
#[derive(Debug, ShaderType, Clone, Component, Copy)]
pub struct MaskUniform {
    pub(crate) strength: f32,
    pub(crate) fade: f32,
}

impl From<Mask> for MaskUniform {
    fn from(mask: Mask) -> Self {
        Self {
            strength: mask.strength,
            fade: mask.fade,
        }
    }
}

impl SyncComponent for Mask {
    type Target = (MaskUniform, EffectShaderDef<Mask>);
}

impl ExtractComponent for Mask {
    type QueryData = (&'static Self, &'static Camera);
    type QueryFilter = ();
    type Out = (MaskUniform, EffectShaderDef<Mask>);

    fn extract_component(
        (settings, camera): QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        if !camera.is_active {
            return None;
        }

        Some((
            (*settings).into(),
            EffectShaderDef::new(settings.variant.shader_def()),
        ))
    }
}
