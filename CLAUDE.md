# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`bevy-vfx-bag` is a library crate of full-screen post-processing effects for Bevy (blur, chromatic aberration, flip, LUT color grading, masks/vignette/CRT, pixelate, raindrops, wave).

Targets **Bevy 0.19**. The README's usage snippet and compatibility table are still from the 0.10 era.

## Commands

```sh
cargo check --lib                     # fastest compile check
cargo build
cargo clippy --all-targets
cargo test --doc                      # the README is included as crate docs (#![doc = include_str!("../README.md")])
cargo r --example                     # list the examples
cargo r --example blur                # run one (examples/<name>/main.rs)
cargo r --example make-neutral-lut    # generates a neutral LUT image to edit for color grading
```

There are no unit tests or CI workflows (the workflows were removed). To verify a change you run the examples. In debug builds Bevy's PBR pipelines take several seconds to compile, so the first frames show only the clear color (with effects applied).

Cargo features `sdr`/`hdr` are declared but unused: pipelines are specialized on each view's `ExtractedView::target_format`.

## Lints enforced in `src/lib.rs`

- `#![deny(missing_docs)]`: every public item needs a doc comment.
- `#![deny(clippy::unwrap_used)]`: use `.expect("<invariant explanation>")` instead of `.unwrap()`.

## Architecture

Bevy 0.19 has no render graph; core rendering is the `Core3d`/`Core2d` schedules. All effects share one render system, `post_processing` in `src/post_processing/mod.rs`, which runs per view in `Core{3d,2d}Systems::PostProcess` after `tonemapping`.

- **`BevyVfxBagPlugin`** (`src/plugin.rs`) only adds `post_processing::PostProcessingPlugin`. That plugin adds each effect's own `Plugin`.
- **Queue**: during `RenderSystems::PrepareBindGroups`, each effect pushes a `QueuedEffect` (order, pipeline id, its bind group 1, dynamic offsets) per view into the `PostProcessingQueue` resource, which is keyed by render-world view entity and cleared in `RenderSystems::Cleanup`. `post_processing` sorts the view's effects by order. For each one it calls `view_target.post_process_write()`, which ping-pongs source/destination, so effects stack in order.
- **Ordering**: `Order<C>` is a per-camera, per-effect component (default `0.0`, lower runs first). It is extracted to the render world. The `PostProcessingOrder` blanket trait adds `effect.order(f32)`, which returns the `(effect, Order<effect>)` bundle.
- **Pipelines**: `EffectPipeline<C>` (generic over the effect component, so each effect has its own `SpecializedRenderPipelines` cache) has layout `[shared_layout(), effect_layout]`. It is specialized on `EffectPipelineKey { target_format, shader_def }`. `shader_def` comes from an optional render-world `EffectShaderDef<C>` component, which masks use to pick `SQUARE`/`CRT`/`VIGNETTE`.
- **Bind group convention**, which every shader must follow:
  - `@group(0)`: shared, built per pass. `binding(0)` is the source texture, `binding(1)` the sampler, `binding(2)` the Bevy `Globals` uniform.
  - `@group(1)`: per effect, usually the effect's uniform with a **dynamic offset** (raindrops and LUT also bind a texture and sampler).
  - Vertex stage: `FullscreenShader`. Fragment entry point: `fragment`, which takes `FullscreenVertexOutput` (`#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput`).

### Anatomy of an effect module (`src/post_processing/<effect>.rs`, see `flip.rs` for the smallest)

1. A public user-facing component, for example `Flip`. Its `SyncComponent` + `ExtractComponent` impls convert it to the render-world uniform (a `#[doc(hidden)]` `<Effect>Uniform: ShaderType`, or the component itself) and skip inactive cameras.
2. A `const <EFFECT>_SHADER_HANDLE: Handle<Shader>` made with `crate::util::weak_from_u64` from a hard-coded random `u64` (use `scripts/random_u64.sh`, and `scripts/print_uuids_u64s.sh` to check that IDs are unique). It is loaded with `load_internal_asset!` from `assets/shaders/<effect>.wgsl`, so shaders are embedded at compile time.
3. `impl FromWorld for EffectPipeline<Effect>` that calls `EffectPipeline::new` with the group 1 layout entries.
4. In the `Plugin`, for uniform-only effects: `super::add_uniform_effect::<Effect, Uniform>(app)`, which registers extraction, `Order<Effect>`, the pipeline, and the generic `queue_uniform_effect` system. Effects with extra bindings (`lut.rs`, `raindrops.rs`) call `add_effect_resources` and register their own queue system.
5. Register the module in `post_processing/mod.rs` (as a `pub mod` with a doc comment, and in `PostProcessingPlugin`'s `add_plugins`).

Users turn an effect on or off by adding or removing its component on a camera. Effects work per camera, so multiple cameras/windows can have independent stacks (see `examples/multi_window`).

### Examples

Each example is a directory `examples/<name>/main.rs` and is auto-discovered by Cargo. Each one shares helpers through `#[path = "../examples_common.rs"] mod examples_common;`. The helpers are `SaneDefaultsPlugin` (default plugins, nearest sampling, ESC to quit), `ShapesExamplePlugin` (a demo 3D scene), and `print_on_change::<T>`. Runtime assets (LUT images, textures) are in `assets/`. `underwater` stacks most effects together.
