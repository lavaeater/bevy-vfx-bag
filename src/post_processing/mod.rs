use std::marker::PhantomData;

use bevy::{
    core_pipeline::{
        schedule::{Core2d, Core2dSystems, Core3d, Core3dSystems},
        tonemapping::tonemapping,
        FullscreenShader,
    },
    ecs::query::QueryItem,
    platform::collections::HashMap,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        globals::{GlobalsBuffer, GlobalsUniform},
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            encase::internal::WriteInto,
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            BindGroupLayoutEntry, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            FilterMode, FragmentState, Operations, PipelineCache, RenderPassColorAttachment,
            RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderStages, ShaderType, SpecializedRenderPipeline,
            SpecializedRenderPipelines, TextureFormat, TextureSampleType,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        sync_component::SyncComponent,
        view::{ExtractedView, ViewTarget},
        GpuResourceAppExt, Render, RenderApp, RenderSystems,
    },
    shader::Shader,
};

/// Blur
pub mod blur;

/// Chromatic Aberration
pub mod chromatic_aberration;

/// Flip
pub mod flip;

/// LUT
pub mod lut;

/// Masks
pub mod masks;

/// Pixelate
pub mod pixelate;

/// Raindrops
pub mod raindrops;

/// Wave
pub mod wave;

/// Adds a `.order` helper method to a component.
/// When used on a post processing effect, it determines the order in which the effect is applied.
///
/// See [`Order`] for more information.
///
/// Also see the multi window example for a usage example.
pub trait PostProcessingOrder: Sized {
    /// Sets the order value on a component, and returns the component
    /// as well as the order as a bundle.
    /// This is therefore meant as a helper when inserting the effect component.
    fn order(self, order: f32) -> (Self, Order<Self>);
}

impl<U> PostProcessingOrder for U
where
    U: Component,
{
    fn order(self, order: f32) -> (Self, Order<Self>) {
        (self, Order::new(order))
    }
}

/// Decide on ordering for post processing effects.
/// Lower numbers means run earlier.
///
/// This is per-camera and uses a generic marker to know which
/// effect to change the order of.
/// It's easier to use this via the [`PostProcessingOrder`] trait
/// which adds a helper method for using this.
#[derive(Debug, Component, Copy)]
pub struct Order<C> {
    /// Priority
    pub order: f32,
    marker: PhantomData<C>,
}

impl<C> Clone for Order<C> {
    fn clone(&self) -> Self {
        Self {
            order: self.order,
            marker: self.marker,
        }
    }
}

impl<C> Order<C> {
    /// Create a new ordering.
    pub fn new(priority: f32) -> Self {
        Self {
            order: priority,
            marker: PhantomData,
        }
    }
}

impl<C: Component> SyncComponent for Order<C> {
    type Target = Self;
}

impl<C: Component> ExtractComponent for Order<C> {
    type QueryData = &'static Self;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(order: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(order.clone())
    }
}

#[doc(hidden)]
/// A shader definition used when specializing the pipeline of effect `C` for a view.
/// Lives in the render world.
#[derive(Debug, Component)]
pub struct EffectShaderDef<C> {
    def: &'static str,
    marker: PhantomData<C>,
}

impl<C> EffectShaderDef<C> {
    pub(crate) fn new(def: &'static str) -> Self {
        Self {
            def,
            marker: PhantomData,
        }
    }
}

/// The bind group layout common to post processing effects (bind group 0).
/// This includes the source texture, its sampler, and the globals uniform.
fn shared_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "PostProcessing shared bind group layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<GlobalsUniform>(false),
            ),
        ),
    )
}

/// Render world resources shared by all post processing effects.
#[derive(Resource)]
struct PostProcessingShared {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
}

impl FromWorld for PostProcessingShared {
    fn from_world(world: &mut World) -> Self {
        let sampler = world
            .resource::<RenderDevice>()
            .create_sampler(&SamplerDescriptor {
                label: Some("PostProcessing texture sampler"),
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                ..default()
            });

        Self {
            layout: shared_layout(),
            sampler,
        }
    }
}

/// The pipeline of the post processing effect `C`.
///
/// Bind group 0 is shared between all effects, see [`shared_layout`].
/// Bind group 1 is specific to the effect.
#[derive(Resource)]
pub(crate) struct EffectPipeline<C> {
    label: &'static str,
    shared_layout: BindGroupLayoutDescriptor,
    pub(crate) effect_layout: BindGroupLayoutDescriptor,
    fullscreen_shader: FullscreenShader,
    shader: Handle<Shader>,
    marker: PhantomData<fn() -> C>,
}

impl<C> EffectPipeline<C> {
    pub(crate) fn new(
        world: &World,
        label: &'static str,
        effect_layout_entries: &[BindGroupLayoutEntry],
        shader: Handle<Shader>,
    ) -> Self {
        Self {
            label,
            shared_layout: shared_layout(),
            effect_layout: BindGroupLayoutDescriptor::new(
                format!("{label} Bind Group Layout"),
                effect_layout_entries,
            ),
            fullscreen_shader: world.resource::<FullscreenShader>().clone(),
            shader,
            marker: PhantomData,
        }
    }

    /// Specializes the pipeline for the given view.
    pub(crate) fn pipeline_id(
        &self,
        pipelines: &mut SpecializedRenderPipelines<Self>,
        pipeline_cache: &PipelineCache,
        view: &ExtractedView,
        shader_def: Option<&EffectShaderDef<C>>,
    ) -> CachedRenderPipelineId
    where
        C: 'static,
    {
        pipelines.specialize(
            pipeline_cache,
            self,
            EffectPipelineKey {
                target_format: view.target_format,
                shader_def: shader_def.map(|d| d.def),
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EffectPipelineKey {
    target_format: TextureFormat,
    shader_def: Option<&'static str>,
}

impl<C: 'static> SpecializedRenderPipeline for EffectPipeline<C> {
    type Key = EffectPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some(format!("{} Render Pipeline", self.label).into()),
            layout: vec![self.shared_layout.clone(), self.effect_layout.clone()],
            vertex: self.fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: key.shader_def.into_iter().map(Into::into).collect(),
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: key.target_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            ..default()
        }
    }
}

/// A post processing pass queued for a view this frame.
pub(crate) struct QueuedEffect {
    /// See [`Order`].
    pub order: f32,
    pub pipeline_id: CachedRenderPipelineId,
    /// Bind group 1, specific to the effect.
    pub bind_group: BindGroup,
    /// Dynamic offsets into bind group 1.
    pub dynamic_offsets: Vec<u32>,
}

/// The post processing passes to run per view entity.
/// Filled by each effect during [`RenderSystems::PrepareBindGroups`], cleared after rendering.
#[derive(Resource, Default)]
pub(crate) struct PostProcessingQueue(HashMap<Entity, Vec<QueuedEffect>>);

impl PostProcessingQueue {
    pub(crate) fn push(&mut self, view: Entity, effect: QueuedEffect) {
        self.0.entry(view).or_default().push(effect);
    }
}

fn clear_queue(mut queue: ResMut<PostProcessingQueue>) {
    queue.0.clear();
}

/// Queues effects whose effect specific bind group only holds the uniform `U`
/// (with a dynamic offset) at binding 0.
#[allow(clippy::type_complexity)]
pub(crate) fn queue_uniform_effect<C, U>(
    mut queue: ResMut<PostProcessingQueue>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<EffectPipeline<C>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<EffectPipeline<C>>>,
    uniforms: Res<ComponentUniforms<U>>,
    views: Query<
        (
            Entity,
            &ExtractedView,
            &DynamicUniformIndex<U>,
            Option<&Order<C>>,
            Option<&EffectShaderDef<C>>,
        ),
        With<U>,
    >,
) where
    C: Component,
    U: Component + ShaderType + WriteInto + Clone,
{
    if views.is_empty() {
        return;
    }

    let Some(uniforms) = uniforms.uniforms().binding() else {
        return;
    };

    let bind_group = render_device.create_bind_group(
        Some(format!("{} Bind Group", pipeline.label).as_str()),
        &pipeline_cache.get_bind_group_layout(&pipeline.effect_layout),
        &BindGroupEntries::single(uniforms),
    );

    for (entity, view, uniform_index, order, shader_def) in &views {
        queue.push(
            entity,
            QueuedEffect {
                order: order.map_or(0.0, |o| o.order),
                pipeline_id: pipeline.pipeline_id(
                    &mut pipelines,
                    &pipeline_cache,
                    view,
                    shader_def,
                ),
                bind_group: bind_group.clone(),
                dynamic_offsets: vec![uniform_index.index()],
            },
        );
    }
}

/// Registers the plugins, resources, and systems needed by an effect `C`
/// which is extracted to the uniform `U` in the render world.
pub(crate) fn add_uniform_effect<C, U>(app: &mut App)
where
    C: ExtractComponent,
    U: Component + ShaderType + WriteInto + Clone,
    EffectPipeline<C>: FromWorld,
{
    app.add_plugins((
        ExtractComponentPlugin::<C>::default(),
        UniformComponentPlugin::<U>::default(),
    ));

    add_effect_resources::<C>(app);

    render_app(app).add_systems(
        Render,
        queue_uniform_effect::<C, U>.in_set(RenderSystems::PrepareBindGroups),
    );
}

/// Registers the parts common to all effects `C`: ordering and the pipeline.
pub(crate) fn add_effect_resources<C>(app: &mut App)
where
    C: Component,
    EffectPipeline<C>: FromWorld,
{
    app.add_plugins(ExtractComponentPlugin::<Order<C>>::default());

    render_app(app)
        .init_gpu_resource::<EffectPipeline<C>>()
        .init_gpu_resource::<SpecializedRenderPipelines<EffectPipeline<C>>>();
}

/// Runs the queued post processing passes of the current view, in order.
fn post_processing(
    view: ViewQuery<(&ViewTarget, Option<&ExtractedCamera>)>,
    queue: Res<PostProcessingQueue>,
    shared: Res<PostProcessingShared>,
    globals: Res<GlobalsBuffer>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let Some(effects) = queue.0.get(&view.entity()) else {
        return;
    };

    let Some(globals) = globals.buffer.binding() else {
        return;
    };

    let (view_target, camera) = view.into_inner();

    let mut effects: Vec<_> = effects.iter().collect();
    effects.sort_by(|a, b| a.order.total_cmp(&b.order));

    let shared_layout = pipeline_cache.get_bind_group_layout(&shared.layout);

    for effect in effects {
        let Some(pipeline) = pipeline_cache.get_render_pipeline(effect.pipeline_id) else {
            continue;
        };

        // Flips the view target's main texture, so we must write to `destination`.
        let post_process = view_target.post_process_write();

        let shared_bind_group = ctx.render_device().create_bind_group(
            "PostProcessing shared bind group",
            &shared_layout,
            &BindGroupEntries::sequential((post_process.source, &shared.sampler, globals.clone())),
        );

        let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("PostProcessing pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        if let Some(viewport) = camera.and_then(|camera| camera.viewport.as_ref()) {
            render_pass.set_camera_viewport(viewport);
        }

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &shared_bind_group, &[]);
        render_pass.set_bind_group(1, &effect.bind_group, &effect.dynamic_offsets);
        // Draw the full screen triangle.
        render_pass.draw(0..3, 0..1);
    }
}

pub(crate) fn render_app(app: &mut App) -> &mut SubApp {
    app.get_sub_app_mut(RenderApp)
        .expect("Need a render app for post processing")
}

#[derive(Debug, Default)]
pub(crate) struct PostProcessingPlugin;

impl Plugin for PostProcessingPlugin {
    fn build(&self, app: &mut App) {
        render_app(app)
            .init_resource::<PostProcessingQueue>()
            .init_gpu_resource::<PostProcessingShared>()
            .add_systems(Render, clear_queue.in_set(RenderSystems::Cleanup))
            // All effects run in this one system, after tonemapping.
            .add_systems(
                Core3d,
                post_processing
                    .after(tonemapping)
                    .in_set(Core3dSystems::PostProcess),
            )
            .add_systems(
                Core2d,
                post_processing
                    .after(tonemapping)
                    .in_set(Core2dSystems::PostProcess),
            );

        app.add_plugins((
            blur::Plugin,
            chromatic_aberration::Plugin,
            flip::Plugin,
            lut::Plugin,
            masks::Plugin,
            raindrops::Plugin,
            pixelate::Plugin,
            wave::Plugin,
        ));
    }
}
