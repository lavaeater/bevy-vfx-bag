use bevy::{
    asset::{load_internal_asset, RenderAssetUsages},
    ecs::query::QueryItem,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_asset::RenderAssets,
        render_resource::{
            binding_types::{sampler, texture_3d},
            BindGroupEntries, BindGroupLayoutEntries, Extent3d, PipelineCache, SamplerBindingType,
            ShaderStages, SpecializedRenderPipelines, TextureDimension, TextureFormat,
            TextureSampleType, TextureViewDescriptor, TextureViewDimension,
        },
        renderer::RenderDevice,
        sync_component::SyncComponent,
        texture::GpuImage,
        view::ExtractedView,
        Render, RenderSystems,
    },
    shader::Shader,
};

use super::{EffectPipeline, Order, PostProcessingQueue, QueuedEffect};

pub(crate) const LUT_SHADER_HANDLE: Handle<Shader> =
    crate::util::weak_from_u64(3719875149378986812);

const LUT_ARCTIC_IMAGE_HANDLE: Handle<Image> = crate::util::weak_from_u64(11514769687270273032);
const LUT_NEO_IMAGE_HANDLE: Handle<Image> = crate::util::weak_from_u64(18411885151390434307);
const LUT_SLATE_IMAGE_HANDLE: Handle<Image> = crate::util::weak_from_u64(8809687374954616573);

impl FromWorld for EffectPipeline<Lut> {
    fn from_world(world: &mut World) -> Self {
        EffectPipeline::new(
            world,
            "LUT",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_3d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
            LUT_SHADER_HANDLE,
        )
    }
}

fn load_lut(assets: &mut Assets<Image>, handle: &Handle<Image>, bytes: &[u8]) {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::Default,
        RenderAssetUsages::default(),
    )
    .expect("Should load LUT successfully");

    assets
        .insert(handle, image)
        .expect("LUT image handle should be valid");
}

pub(crate) struct Plugin;
impl bevy::prelude::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            LUT_SHADER_HANDLE,
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/shaders/", "lut.wgsl"),
            Shader::from_wgsl
        );

        let mut assets = app.world_mut().resource_mut::<Assets<Image>>();

        load_lut(
            &mut assets,
            &LUT_NEO_IMAGE_HANDLE,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/luts/",
                "neo.png"
            )),
        );
        load_lut(
            &mut assets,
            &LUT_SLATE_IMAGE_HANDLE,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/luts/",
                "slate.png"
            )),
        );
        load_lut(
            &mut assets,
            &LUT_ARCTIC_IMAGE_HANDLE,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/luts/",
                "arctic.png"
            )),
        );

        app.add_plugins(ExtractComponentPlugin::<Lut>::default())
            .add_systems(PostUpdate, adapt_image_for_lut_use);

        super::add_effect_resources::<Lut>(app);

        super::render_app(app)
            .add_systems(Render, queue_lut.in_set(RenderSystems::PrepareBindGroups));
    }
}

fn adapt_image_for_lut_use(mut assets: ResMut<Assets<Image>>, mut luts: Query<&mut Lut>) {
    for mut lut in luts.iter_mut() {
        if lut.prepared {
            continue;
        }

        // The image may still be loading, in which case we try again next frame.
        let Some(mut image) = assets.get_mut(&lut.texture) else {
            continue;
        };

        // The LUT is a 3d texture. It has 64 layers, each of which is a 64x64 image.
        image.texture_descriptor.size = Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 64,
        };
        image.texture_descriptor.dimension = TextureDimension::D3;
        image.texture_descriptor.format = TextureFormat::Rgba8Unorm;

        image.texture_view_descriptor = Some(TextureViewDescriptor {
            label: Some("LUT Texture View"),
            format: Some(TextureFormat::Rgba8Unorm),
            dimension: Some(TextureViewDimension::D3),
            ..default()
        });

        debug!("LUT prepared for handle {:?}", lut.texture);
        lut.prepared = true;
    }
}

fn queue_lut(
    mut queue: ResMut<PostProcessingQueue>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<EffectPipeline<Lut>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<EffectPipeline<Lut>>>,
    images: Res<RenderAssets<GpuImage>>,
    views: Query<(Entity, &ExtractedView, &Lut, Option<&Order<Lut>>)>,
) {
    for (entity, view, lut, order) in &views {
        let Some(lut_image) = images.get(&lut.texture) else {
            continue;
        };

        let bind_group = render_device.create_bind_group(
            "LUT Bind Group",
            &pipeline_cache.get_bind_group_layout(&pipeline.effect_layout),
            &BindGroupEntries::sequential((&lut_image.texture_view, &lut_image.sampler)),
        );

        queue.push(
            entity,
            QueuedEffect {
                order: order.map_or(0.0, |o| o.order),
                pipeline_id: pipeline.pipeline_id(&mut pipelines, &pipeline_cache, view, None),
                bind_group,
                dynamic_offsets: vec![],
            },
        );
    }
}

/// A look-up texture. Maps colors to colors. Useful for colorschemes.
#[derive(Debug, Component, Clone)]
pub struct Lut {
    /// The 3D look-up texture
    texture: Handle<Image>,

    prepared: bool,
}

impl Lut {
    /// Creates a new LUT component.
    /// The image should be a 64x64x64 3D texture.
    /// See the `make-neutral-lut` example.
    pub fn new(texture: Handle<Image>) -> Self {
        Self {
            texture,
            prepared: false,
        }
    }

    /// The arctic color scheme LUT.
    pub fn arctic() -> Self {
        Self::new(LUT_ARCTIC_IMAGE_HANDLE)
    }

    /// The neo color scheme LUT.
    pub fn neo() -> Self {
        Self::default()
    }

    /// The slate color scheme LUT.
    pub fn slate() -> Self {
        Self::new(LUT_SLATE_IMAGE_HANDLE)
    }
}

impl Default for Lut {
    fn default() -> Self {
        Self::new(LUT_NEO_IMAGE_HANDLE)
    }
}

impl SyncComponent for Lut {
    type Target = Self;
}

impl ExtractComponent for Lut {
    type QueryData = (&'static Self, &'static Camera);
    type QueryFilter = ();
    type Out = Self;

    fn extract_component((lut, camera): QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        if !camera.is_active || !lut.prepared {
            return None;
        }

        Some(lut.clone())
    }
}
