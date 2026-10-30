# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`bevy-vfx-bag` is a library crate of full-screen post-processing effects for Bevy (blur, chromatic aberration, flip, LUT color grading, masks/vignette/CRT, pixelate, raindrops, wave).

**Current state (branch `upgrade-bevy`):** `Cargo.toml` was bumped to Bevy 0.19, but all source code is still written against the **Bevy 0.10** API (`HandleUntyped`/`TypeUuid`, `add_plugin`/`add_system`, `RenderSet`, `RenderPhase`, `render_graph` slot edges, `ExtractComponent::Query/Filter`, `RenderCommand::ViewWorldQuery/ItemWorldQuery`, `core_3d::graph::NAME`). The crate does not compile yet (~120 errors from `cargo check --lib`). Porting means rewriting the render-graph and phase plumbing in `src/post_processing/mod.rs` and `src/util.rs`, then doing the same for every effect module. The examples use 0.10 APIs too (`add_startup_system`, `close_on_esc`, `AssetPlugin { watch_for_changes }`). The README's usage snippet and compatibility table are also from the 0.10 era.

## Commands

```sh
cargo check --lib                     # fastest way to see port/compile errors
cargo build
cargo clippy --all-targets
cargo test --doc                      # the README is included as crate docs (#![doc = include_str!("../README.md")])
cargo r --example                     # list the examples
cargo r --example blur                # run one (examples/<name>/main.rs)
cargo r --example make-neutral-lut    # generates a neutral LUT image to edit for color grading
cargo build --no-default-features --features hdr   # HDR target format instead of SDR
```

There are no unit tests or CI workflows (the workflows were removed). To verify a change you run the examples.

Cargo features: `sdr` (default) and `hdr` choose the pipeline's fragment target format in `render_pipeline_descriptor` (`TextureFormat::bevy_default()` vs `ViewTarget::TEXTURE_FORMAT_HDR`). Enable exactly one: if you enable both, the `targets` field is duplicated, and if you enable neither, it is missing.

## Lints enforced in `src/lib.rs`

- `#![deny(missing_docs)]`: every public item needs a doc comment.
- `#![deny(clippy::unwrap_used)]`: use `.expect("<invariant explanation>")` instead of `.unwrap()`.

## Architecture

All effects share one custom render phase and one render-graph node, and they run in sequence on the camera's view target.

- **`BevyVfxBagPlugin`** (`src/plugin.rs`) only adds `post_processing::PostProcessingPlugin`. That plugin adds each effect's own `Plugin`.
- **Graph wiring** (`src/util.rs::add_nodes`): a single `PostProcessingNode` goes into both the 2D and 3D core sub-graphs, between the main pass and `END_MAIN_PASS_POST_PROCESSING`.
- **Phase** (`src/post_processing/mod.rs`): `PostProcessingPhaseItem` is sorted by `FloatOrd`. For each item, `PostProcessingNode::run` calls `view_target.post_process_write()`, which ping-pongs source/destination, so effects stack in sort order.
- **Ordering**: `Order<C>` is a per-camera, per-effect component (default `0.0`, lower runs first). The `PostProcessingOrder` blanket trait adds `effect.order(f32)`, which returns the `(effect, Order<effect>)` bundle.
- **Bind group convention**, which every shader must follow:
  - `@group(0)`: shared. `binding(0)` is the source texture, `binding(1)` the sampler, `binding(2)` the Bevy `Globals` uniform. Bind groups are cached per `TextureViewId` in `PostProcessingSharedBindGroups`. The current source id is passed to the render command through a `Mutex`.
  - `@group(1)`: per effect. `binding(0)` is the effect's uniform with a **dynamic offset**, and later bindings are optional extras (for example the LUT texture).
  - Vertex stage: Bevy's `fullscreen_shader_vertex_state()`. Fragment entry point: `fragment`, which takes `FullscreenVertexOutput`.
- **Draw function**: `DrawPostProcessingEffect<U>` = `SetItemPipeline` → `SetTextureSamplerGlobals<0>` → `SetEffectBindGroup<U, 1>` → `DrawPostProcessing` (a 3-vertex fullscreen triangle).

### Anatomy of an effect module (`src/post_processing/<effect>.rs`, see `flip.rs` for the smallest)

1. A public user-facing component, for example `Flip`. Its `ExtractComponent` impl converts it to a `#[doc(hidden)]` `<Effect>Uniform: ShaderType` and skips inactive cameras.
2. A `const <EFFECT>_SHADER_HANDLE`, a weak handle from a hard-coded random `u64` (use `scripts/random_u64.sh`, and `scripts/print_uuids_u64s.sh` to check that IDs are unique). It is loaded with `load_internal_asset!` from `assets/shaders/<effect>.wgsl`, so shaders are embedded at compile time.
3. An `<Effect>Data` resource (`FromWorld`) holding the uniform `BindGroupLayout` and `CachedRenderPipelineId`, built through `super::create_layout_and_pipeline`.
4. In the `Plugin`: `ExtractComponentPlugin` + `UniformComponentPlugin` for the uniform. In the render app: `extract_post_processing_camera_phases::<Effect>` in Extract, `prepare` (adds a phase item per camera that has `Order<Effect>`), `queue` (rebuilds the `UniformBindGroup<Uniform>`), and `add_render_command::<PostProcessingPhaseItem, DrawPostProcessingEffect<Uniform>>()`.
5. Register the module in `post_processing/mod.rs` (as a `pub mod` with a doc comment, and with `app.add_plugin(...)` in `PostProcessingPlugin`).

Users turn an effect on or off by adding or removing its component on a camera. Effects work per camera, so multiple cameras/windows can have independent stacks (see `examples/multi_window`).

### Examples

Each example is a directory `examples/<name>/main.rs` and is auto-discovered by Cargo. Each one shares helpers through `#[path = "../examples_common.rs"] mod examples_common;`. The helpers are `SaneDefaultsPlugin` (default plugins, asset hot reload, ESC to quit), `ShapesExamplePlugin` (a demo 3D scene), and `print_on_change::<T>`. Runtime assets (LUT images, textures) are in `assets/`. `underwater` stacks most effects together.
