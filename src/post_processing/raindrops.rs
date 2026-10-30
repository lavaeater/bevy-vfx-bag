use std::fmt::Display;

use bevy::{
    asset::{load_internal_asset, RenderAssetUsages},
    ecs::query::QueryItem,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            AddressMode, BindGroupEntries, BindGroupLayoutEntries, PipelineCache, Sampler,
            SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType,
            SpecializedRenderPipelines, TextureSampleType,
        },
        renderer::RenderDevice,
        sync_component::SyncComponent,
        texture::GpuImage,
        view::ExtractedView,
        GpuResourceAppExt, Render, RenderSystems,
    },
    shader::Shader,
};

use super::{EffectPipeline, Order, PostProcessingQueue, QueuedEffect};

pub(crate) const RAINDROPS_SHADER_HANDLE: Handle<Shader> =
    crate::util::weak_from_u64(3481202994982538867);
const RAINDROPS_IMAGE_HANDLE: Handle<Image> = crate::util::weak_from_u64(10600833861652934799);

impl FromWorld for EffectPipeline<Raindrops> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "Raindrops",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<Raindrops>(true),
                ),
            ),
            RAINDROPS_SHADER_HANDLE,
        )
    }
}

/// Repeating sampler for the raindrops texture.
#[derive(Resource)]
struct RaindropsSampler(Sampler);

impl FromWorld for RaindropsSampler {
    fn from_world(world: &mut World) -> Self {
        Self(
            world
                .resource::<RenderDevice>()
                .create_sampler(&SamplerDescriptor {
                    label: Some("Raindrops Sampler"),
                    address_mode_u: AddressMode::Repeat,
                    address_mode_v: AddressMode::Repeat,
                    address_mode_w: AddressMode::Repeat,
                    ..default()
                }),
        )
    }
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            RAINDROPS_SHADER_HANDLE,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/shaders/",
                "raindrops.wgsl"
            ),
            Shader::from_wgsl
        );

        let image = Image::from_buffer(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/textures/",
                "raindrops.tga"
            )),
            ImageType::Extension("tga"),
            CompressedImageFormats::NONE,
            false,
            ImageSampler::Default,
            RenderAssetUsages::default(),
        )
        .expect("Should load raindrops successfully");
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .insert(&RAINDROPS_IMAGE_HANDLE, image)
            .expect("Raindrops image handle should be valid");

        // This puts the uniform into the render world.
        app.add_plugins((
            ExtractComponentPlugin::<Raindrops>::default(),
            UniformComponentPlugin::<Raindrops>::default(),
        ));

        super::add_effect_resources::<Raindrops>(app);

        super::render_app(app)
            .init_gpu_resource::<RaindropsSampler>()
            .add_systems(
                Render,
                queue_raindrops.in_set(RenderSystems::PrepareBindGroups),
            );
    }
}

#[allow(clippy::type_complexity)]
fn queue_raindrops(
    mut queue: ResMut<PostProcessingQueue>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<EffectPipeline<Raindrops>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<EffectPipeline<Raindrops>>>,
    raindrops_sampler: Res<RaindropsSampler>,
    uniforms: Res<ComponentUniforms<Raindrops>>,
    images: Res<RenderAssets<GpuImage>>,
    views: Query<
        (
            Entity,
            &ExtractedView,
            &DynamicUniformIndex<Raindrops>,
            Option<&Order<Raindrops>>,
        ),
        With<Raindrops>,
    >,
) {
    if views.is_empty() {
        return;
    }

    let (Some(uniforms), Some(raindrops_image)) = (
        uniforms.uniforms().binding(),
        images.get(&RAINDROPS_IMAGE_HANDLE),
    ) else {
        return;
    };

    let bind_group = render_device.create_bind_group(
        "Raindrops Bind Group",
        &pipeline_cache.get_bind_group_layout(&pipeline.effect_layout),
        &BindGroupEntries::sequential((
            &raindrops_image.texture_view,
            &raindrops_sampler.0,
            uniforms,
        )),
    );

    for (entity, view, uniform_index, order) in &views {
        queue.push(
            entity,
            QueuedEffect {
                order: order.map_or(0.0, |o| o.order),
                pipeline_id: pipeline.pipeline_id(&mut pipelines, &pipeline_cache, view, None),
                bind_group: bind_group.clone(),
                dynamic_offsets: vec![uniform_index.index()],
            },
        );
    }
}

/// Raindrops settings.
#[derive(Debug, Component, Clone, Copy, ShaderType)]
pub struct Raindrops {
    /// How quickly the raindrops animate.
    pub speed: f32,

    /// How much the raindrops warp the image.
    pub warping: f32,

    /// How zoomed in the raindrops texture is.
    pub zoom: f32,
}

impl Default for Raindrops {
    fn default() -> Self {
        Self {
            speed: 0.8,
            warping: 0.03,
            zoom: 1.0,
        }
    }
}

impl Display for Raindrops {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Raindrops speed: {}, warping: {}, zoom: {}",
            self.speed, self.warping, self.zoom
        )
    }
}

impl SyncComponent for Raindrops {
    type Target = Self;
}

impl ExtractComponent for Raindrops {
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
