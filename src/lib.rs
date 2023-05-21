#[cfg(target_os = "windows")]
use ::windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS,
};
use anyhow::Context;
use cgmath::Rotation3;
use clap::Parser;
use config::{AppConfig, TemporalBlurParams};
use engine::{
    camera::{Camera, CameraUniform},
    geometry::{ModelVertex, Vertex},
    input::InputContext,
    screen::Screen,
    texture::{Bound, RoundRobinTextureBuffer, Texture2D, Unbound},
    vr::{enable_xr_runtime, OpenXRContext, SWAPCHAIN_COLOR_FORMAT, VIEW_COUNT, VIEW_TYPE},
    WgpuContext, WgpuLoader,
};
use loaders::{blank_loader::BlankLoader, Loader, StereoMode};
use log::error;
use utils::commands::AppState;

use openxr::ReferenceSpaceType;
use std::{
    iter,
    num::NonZeroU32,
    sync::{Arc, Mutex},
};
use thread_priority::*;
use wgpu::{util::DeviceExt, BindGroupLayout};

use crate::{
    config::ConfigContext,
    utils::commands::{AppCommands, AppContext, RecenterRequest, ToggleSetting},
};

mod config;
mod conversions;
mod engine;
mod loaders;
mod utils;

#[macro_use]
mod macros;

const AMBIENT_BLUR_BASE_RES: u32 = 16;
const AMBIENT_BLUR_TEMPORAL_SAMPLES: u32 = 16;
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

pub fn launch() -> anyhow::Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    #[cfg(feature = "profiling")]
    tracy_client::Client::start();

    #[cfg(feature = "profiling")]
    profiling::register_thread!("Main Thread");

    #[cfg(feature = "renderdoc")]
    let _rd: renderdoc::RenderDoc<renderdoc::V110> =
        renderdoc::RenderDoc::new().context("Unable to connect to renderdoc")?;

    #[cfg(not(target_os = "android"))]
    utils::logging::setup_logging()?;

    try_elevate_priority();

    let app = AppContext::new()?;
    let mut xr_context = enable_xr_runtime()?;
    let wgpu_context = xr_context.load_wgpu()?;

    let mut config_context = config::ConfigContext::try_setup().unwrap_or(None);

    log::info!("Finished initial setup, running main loop");
    run(
        &mut xr_context,
        &wgpu_context,
        &app.state,
        &mut config_context,
    )?;

    Ok(())
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn try_elevate_priority() {
    log::info!("Trying to elevate process priority");
    if set_current_thread_priority(ThreadPriority::Max).is_err() {
        log::warn!("Failed to set thread priority to max!");
    }

    #[cfg(target_os = "windows")]
    {
        if !unsafe { SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS) }.as_bool() {
            log::warn!("Failed to set process priority to max!");
        }
    }
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn run(
    xr_context: &mut OpenXRContext,
    wgpu_context: &WgpuContext,
    app_state: &Arc<Mutex<AppState>>,
    config: &mut Option<ConfigContext>,
) -> anyhow::Result<()> {
    // Load the shaders from disk
    let screen_shader = wgpu_context
        .device
        .create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
    let blit_shader = wgpu_context
        .device
        .create_shader_module(wgpu::include_wgsl!("blit.wgsl"));

    // We don't need to configure the texture view much, so let's
    // let wgpu define it.

    let mut stereo_mode = StereoMode::Mono;
    let default_stereo_mode = StereoMode::Mono; // Not configurable for now
    let mut current_loader = None;

    let mut loaders: Vec<Box<dyn Loader>> = vec![
        #[cfg(target_os = "windows")]
        {
            use loaders::katanga_loader::KatangaLoaderContext;
            Box::<KatangaLoaderContext>::default()
        },
        #[cfg(target_os = "windows")]
        {
            use loaders::desktop_duplication_loader::DesktopDuplicationLoader;
            Box::new(DesktopDuplicationLoader::new(0)?)
        },
        #[cfg(any(target_os = "unix"))]
        {
            use loaders::captrs_loader::CaptrLoader;
            Box::new(CaptrLoader::new(0)?)
        },
        Box::<BlankLoader>::default(),
    ];

    let texture_bind_group_layout =
        wgpu_context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

    let mut screen_texture = loaders
        .last_mut()
        .unwrap()
        .load(
            &wgpu_context.instance,
            &wgpu_context.device,
            &wgpu_context.queue,
        )?
        .texture
        .bind_to_context(wgpu_context, &texture_bind_group_layout);

    let mut ambient_texture = get_ambient_texture(
        &screen_texture,
        1.0,
        &StereoMode::Mono,
        wgpu_context,
        &texture_bind_group_layout,
    )?;

    let fullscreen_triangle_index_buffer =
        wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Fullscreen Tri Index Buffer"),
                contents: bytemuck::cast_slice(&[0, 1, 2]),
                usage: wgpu::BufferUsages::INDEX,
            });

    let mut screen_params = match config {
        Some(ConfigContext {
            last_config: Some(config),
            ..
        }) => config.clone(),
        _ => AppConfig::parse(),
    };

    let mut temporal_blur_params = TemporalBlurParams {
        jitter: [0.0, 0.0],
        scale: [1.1, 1.1],
        resolution: [
            ambient_texture.current().texture.width() as f32,
            ambient_texture.current().texture.height() as f32,
        ],
        history_decay: 0.985,
    };

    let mut screen = Screen::new(
        &wgpu_context.device,
        -screen_params.distance,
        screen_params.scale,
        1.0,
        screen_params.ambient,
    );

    let screen_params_buffer =
        wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Screen Params Buffer"),
                contents: bytemuck::cast_slice(&[screen_params.uniform(1.0, 1, 1, &stereo_mode)]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

    let temporal_blur_params_buffer =
        wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Temporal Blur Params Buffer"),
                contents: bytemuck::cast_slice(&[temporal_blur_params.uniform()]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

    let screen_model_matrix_buffer =
        wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Screen Model Matrix Buffer"),
                contents: bytemuck::cast_slice(&[screen.entity.uniform()]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

    let mut cameras = vec![Camera::default(), Camera::default()];
    let mut camera_uniform = vec![CameraUniform::new(), CameraUniform::new()];

    let camera_buffer = wgpu_context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(camera_uniform.as_slice()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    let global_temporal_blur_uniform_layout =
        wgpu_context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("global_temporal_blur_bind_group_layout"),
            });

    let global_uniform_bind_group_layout =
        wgpu_context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("global_uniform_bind_group_layout"),
            });

    let global_uniform_bind_group =
        wgpu_context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &global_uniform_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: screen_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: screen_model_matrix_buffer.as_entire_binding(),
                    },
                ],
                label: Some("global_uniform_bind_group"),
            });

    let global_temporal_blur_uniform_bind_group =
        wgpu_context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &global_temporal_blur_uniform_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: temporal_blur_params_buffer.as_entire_binding(),
                }],
                label: Some("global_temporal_blur_uniform_bind_group"),
            });

    let render_pipeline_layout =
        wgpu_context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &global_uniform_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

    let blit_pipeline_layout =
        wgpu_context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Blit Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &texture_bind_group_layout,
                    &global_temporal_blur_uniform_layout,
                ],
                push_constant_ranges: &[],
            });

    let screen_render_pipeline =
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &screen_shader,
                    entry_point: "vs_main",
                    buffers: &[ModelVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &screen_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SWAPCHAIN_COLOR_FORMAT.try_into()?,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: NonZeroU32::new(VIEW_COUNT),
            });

    let ambient_dome_pipeline =
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Ambient Dome Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &screen_shader,
                    entry_point: "mv_vs_main",
                    buffers: &[ModelVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &screen_shader,
                    entry_point: "vignette_fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SWAPCHAIN_COLOR_FORMAT.try_into()?,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: NonZeroU32::new(VIEW_COUNT),
            });

    let temporal_blur_pipeline =
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Blit Pipeline"),
                layout: Some(&blit_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &blit_shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &blit_shader,
                    entry_point: "temporal_fs_main",
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format: SWAPCHAIN_COLOR_FORMAT.try_into()?,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                        Some(wgpu::ColorTargetState {
                            format: SWAPCHAIN_COLOR_FORMAT.try_into()?,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

    // Start the OpenXR session
    let (xr_session, mut frame_wait, mut frame_stream) = unsafe {
        xr_context.instance.create_session::<openxr::Vulkan>(
            xr_context.system,
            &openxr::vulkan::SessionCreateInfo {
                instance: wgpu_context.vk_instance_ptr as _,
                physical_device: wgpu_context.vk_phys_device_ptr as _,
                device: wgpu_context.vk_device_ptr as _,
                queue_family_index: wgpu_context.queue_index,
                queue_index: 0,
            },
        )?
    };

    // Create a room-scale reference space
    let xr_reference_space = xr_session
        .create_reference_space(openxr::ReferenceSpaceType::LOCAL, openxr::Posef::IDENTITY)?;
    let xr_view_space = xr_session
        .create_reference_space(openxr::ReferenceSpaceType::VIEW, openxr::Posef::IDENTITY)?;
    let mut xr_space = xr_session
        .create_reference_space(openxr::ReferenceSpaceType::LOCAL, openxr::Posef::IDENTITY)?;

    let mut event_storage = openxr::EventDataBuffer::new();
    let mut session_running = false;
    let mut swapchain = None;
    let mut screen_invalidated = false;
    let mut recenter_request = None;
    let mut last_invalidation_check = std::time::Instant::now();
    let mut last_upgrade_check = std::time::Instant::now();
    let mut input_context = InputContext::init(&xr_context.instance)
        .map(Some)
        .unwrap_or(None);

    if input_context.is_some() {
        let mut attach_context = input_context
            .take()
            .context("Cannot attach input context to session")?;
        if attach_context.attach_to_session(&xr_session).is_ok() {
            input_context = Some(attach_context);
        }
    }

    let mut jitter_frame: u32 = 0;
    // Handle OpenXR events
    loop {
        #[cfg(feature = "profiling")]
        profiling::scope!("main loop");

        let time = std::time::Instant::now();

        // Try to upgrade the loader to one that has higher priority
        if current_loader.is_none() || time.duration_since(last_upgrade_check).as_secs() > 10 {
            #[cfg(feature = "profiling")]
            profiling::scope!("Loader Upgrade");

            if let Some((texture, aspect, mode, loader)) =
                try_to_load_texture(&mut loaders, wgpu_context, current_loader)
            {
                let mode = mode.unwrap_or(default_stereo_mode.clone());
                screen_texture = texture.bind_to_context(wgpu_context, &texture_bind_group_layout);
                ambient_texture = get_ambient_texture(
                    &screen_texture,
                    aspect,
                    &mode,
                    wgpu_context,
                    &texture_bind_group_layout,
                )?;
                screen.change_aspect_ratio(aspect);
                stereo_mode = mode;
                screen_invalidated = current_loader != Some(loader);
                current_loader = Some(loader);
            }

            last_upgrade_check = time;
        }

        // Check if the loader needs to be invalidated
        if current_loader.is_some() || time.duration_since(last_invalidation_check).as_secs() > 10 {
            #[cfg(feature = "profiling")]
            profiling::scope!("Loader Invalidation Check");

            check_loader_invalidation(current_loader, &loaders, &mut screen_invalidated)?;
            last_invalidation_check = time;
        }

        // Check if the loader has been invalidated
        if screen_invalidated {
            #[cfg(feature = "profiling")]
            profiling::scope!("Loader Invalidation");

            // Try to load a new texture from the same loader, or from any loader if the current one fails
            let new_loader = current_loader
                .map(|loader_idx| (loaders.get_mut(loader_idx), loader_idx))
                .filter(|(loader, _)| loader.is_some())
                .map(|(loader, loader_idx)| try_loader(loader.unwrap(), wgpu_context, loader_idx))
                .map(|loader| {
                    if loader.is_some() {
                        loader
                    } else {
                        try_to_load_texture(&mut loaders, wgpu_context, None)
                    }
                })
                .unwrap_or_default();

            if let Some((texture, aspect, mode, loader)) = new_loader {
                let mode = mode.unwrap_or(default_stereo_mode.clone());
                screen_texture = texture.bind_to_context(wgpu_context, &texture_bind_group_layout);
                ambient_texture = get_ambient_texture(
                    &screen_texture,
                    aspect,
                    &mode,
                    wgpu_context,
                    &texture_bind_group_layout,
                )?;
                screen.change_aspect_ratio(aspect);
                current_loader = Some(loader);
                stereo_mode = mode;

                wgpu_context.queue.write_buffer(
                    &screen_model_matrix_buffer,
                    0,
                    bytemuck::cast_slice(&[screen.entity.uniform()]),
                );

                let width_multiplier = match &stereo_mode {
                    StereoMode::FullSbs => 2,
                    _ => 1,
                };

                wgpu_context.queue.write_buffer(
                    &screen_params_buffer,
                    0,
                    bytemuck::cast_slice(&[screen_params.uniform(
                        aspect,
                        screen_texture.texture.width() * width_multiplier,
                        ambient_texture.current().texture.width() * width_multiplier,
                        &stereo_mode,
                    )]),
                );
            }
            screen_invalidated = false;
        }

        // Run loader update logic
        if let Some(current_loader) = current_loader {
            #[cfg(feature = "profiling")]
            profiling::scope!("Loader Update");

            if let Err(error) = loaders
                .get_mut(current_loader)
                .map(|loader| {
                    loader.update(
                        &wgpu_context.instance,
                        &wgpu_context.device,
                        &wgpu_context.queue,
                        &screen_texture,
                    )
                })
                .unwrap_or(Err(anyhow::anyhow!("Loader not found")))
            {
                screen_invalidated = true;
                error!("Loader update failed: {}", error);
            }
        }

        let event = xr_context.instance.poll_event(&mut event_storage)?;
        match event {
            Some(openxr::Event::SessionStateChanged(e)) => {
                // Session state change is where we can begin and end sessions, as well as
                // find quit messages!
                log::info!("Entered state {:?}", e.state());
                match e.state() {
                    openxr::SessionState::READY => {
                        xr_session.begin(VIEW_TYPE)?;
                        session_running = true;
                    }
                    openxr::SessionState::STOPPING => {
                        xr_session.end()?;
                        session_running = false;
                    }
                    openxr::SessionState::EXITING => {
                        break;
                    }
                    _ => {}
                }
            }
            Some(openxr::Event::InstanceLossPending(_)) => {}
            Some(openxr::Event::EventsLost(e)) => {
                log::error!("Lost {} OpenXR events", e.lost_event_count());
            }
            Some(openxr::Event::ReferenceSpaceChangePending(_)) => {
                //Reset XR space to follow runtime
                xr_space = xr_session.create_reference_space(
                    openxr::ReferenceSpaceType::LOCAL,
                    openxr::Posef::IDENTITY,
                )?;
            }
            _ => {
                // Render to HMD only if we have an active session
                if session_running {
                    // Block until the previous frame is finished displaying, and is ready for
                    // another one. Also returns a prediction of when the next frame will be
                    // displayed, for use with predicting locations of controllers, viewpoints, etc.
                    let xr_frame_state = {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Wait for frame");

                        frame_wait.wait()?
                    };

                    // If we do not have a swapchain yet, create it
                    let (xr_swapchain, resolution, swapchain_textures) = {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Swapchain Setup");

                        match swapchain {
                            Some(ref mut swapchain) => swapchain,
                            None => {
                                let new_swapchain = xr_context
                                    .create_swapchain(&xr_session, &wgpu_context.device)?;
                                swapchain.get_or_insert(new_swapchain)
                            }
                        }
                    };
                    // Check which image we need to render to and wait until the compositor is
                    // done with this image
                    let image_index = xr_swapchain.acquire_image()?;
                    {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Swapchain Wait");

                        xr_swapchain.wait_image(openxr::Duration::INFINITE)?;
                    }

                    let swapchain_view = &swapchain_textures[image_index as usize].view;

                    // Must be called before any rendering is done!
                    {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Begin FrameStream");
                        frame_stream.begin()?;
                    }

                    #[cfg(feature = "profiling")]
                    profiling::scope!("FrameStream Recording");

                    // Only render if we should
                    if !xr_frame_state.should_render {
                        #[cfg(feature = "profiling")]
                        {
                            let predicted_display_time_nanos =
                                xr_frame_state.predicted_display_time.as_nanos();
                            profiling::scope!(
                                "Show Time Calculation",
                                format!("{predicted_display_time_nanos}").as_str()
                            );
                        }

                        xr_swapchain.release_image()?;

                        // Early bail
                        if let Err(err) = frame_stream.end(
                            xr_frame_state.predicted_display_time,
                            xr_context.blend_mode,
                            &[],
                        ) {
                            log::error!(
                                "Failed to end frame stream when should_render is FALSE : {:?}",
                                err
                            );
                        };
                        continue;
                    }

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Encode Render Passes");

                    // Render!
                    let mut encoder = wgpu_context.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encorder"),
                        },
                    );

                    if let Some(loader) = current_loader.and_then(|index| loaders.get(index)) {
                        loader.encode_pre_pass(&mut encoder, &screen_texture)?;
                    }

                    if screen.ambient_enabled {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Encode Ambient Pass");

                        ambient_texture.next();
                        let mut blit_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Blit Pass"),
                                color_attachments: &[
                                    Some(wgpu::RenderPassColorAttachment {
                                        view: &ambient_texture.current().view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                            store: true,
                                        },
                                    }),
                                    Some(wgpu::RenderPassColorAttachment {
                                        view: &ambient_texture.previous(1).view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                            store: true,
                                        },
                                    }),
                                ],
                                depth_stencil_attachment: None,
                            });

                        blit_pass.set_pipeline(&temporal_blur_pipeline);
                        blit_pass.set_bind_group(0, screen_texture.bind_group(), &[]);
                        blit_pass.set_bind_group(1, ambient_texture.previous(2).bind_group(), &[]);
                        blit_pass.set_bind_group(2, &global_temporal_blur_uniform_bind_group, &[]);
                        blit_pass.set_index_buffer(
                            fullscreen_triangle_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        blit_pass.draw_indexed(0..3, 0, 0..1);
                    }
                    {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Encode Render Pass");
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: swapchain_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: true,
                                },
                            })],
                            depth_stencil_attachment: None,
                        });

                        // Render the ambient dome
                        if screen.ambient_enabled {
                            let ambient_mesh = &screen.ambient_mesh;
                            rpass.set_pipeline(&ambient_dome_pipeline);
                            rpass.set_bind_group(0, ambient_texture.current().bind_group(), &[]);
                            rpass.set_bind_group(1, &global_uniform_bind_group, &[]);
                            rpass.set_vertex_buffer(0, ambient_mesh.vertex_buffer().slice(..));
                            rpass.set_index_buffer(
                                ambient_mesh.index_buffer().slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            rpass.draw_indexed(0..ambient_mesh.indices(), 0, 0..1);
                        }

                        // Render the screen
                        rpass.set_pipeline(&screen_render_pipeline);
                        rpass.set_bind_group(0, screen_texture.bind_group(), &[]);
                        rpass.set_bind_group(1, &global_uniform_bind_group, &[]);
                        rpass.set_vertex_buffer(0, screen.mesh.vertex_buffer().slice(..));
                        rpass.set_index_buffer(
                            screen.mesh.index_buffer().slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        rpass.draw_indexed(0..screen.mesh.indices(), 0, 0..1);
                    }

                    if screen.ambient_enabled {
                        upload_blur_uniforms(
                            &ambient_texture,
                            &mut jitter_frame,
                            &mut temporal_blur_params,
                            wgpu_context,
                            &temporal_blur_params_buffer,
                        );
                    }

                    // Fetch the view transforms. To minimize latency, we intentionally do this
                    // *after* recording commands to render the scene, i.e. at the last possible
                    // moment before rendering begins in earnest on the GPU. Uniforms dependent on
                    // this data can be sent to the GPU just-in-time by writing them to per-frame
                    // host-visible memory which the GPU will only read once the command buffer is
                    // submitted.
                    log::trace!("Locate views");
                    let (_, views) = {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Locate Views");
                        xr_session.locate_views(
                            VIEW_TYPE,
                            xr_frame_state.predicted_display_time,
                            &xr_space,
                        )?
                    };

                    upload_camera_uniforms(
                        &views,
                        &mut cameras,
                        &mut camera_uniform,
                        wgpu_context,
                        &camera_buffer,
                    )?;

                    log::trace!("Submit command buffer");
                    {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Encoder Submit");
                        wgpu_context.queue.submit(iter::once(encoder.finish()));
                    }

                    log::trace!("Release swapchain image");
                    {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Release Swapchain");

                        xr_swapchain.release_image()?;
                    }

                    log::trace!("End frame stream");
                    {
                        #[cfg(feature = "profiling")]
                        let predicted_display_time_nanos =
                            xr_frame_state.predicted_display_time.as_nanos();
                        #[cfg(feature = "profiling")]
                        profiling::scope!(
                            "End Frame",
                            format!("{predicted_display_time_nanos}").as_str()
                        );

                        // End rendering and submit the images
                        let rect = openxr::Rect2Di {
                            offset: openxr::Offset2Di { x: 0, y: 0 },
                            extent: openxr::Extent2Di {
                                width: resolution.width as _,
                                height: resolution.height as _,
                            },
                        };

                        if let Err(err) = frame_stream.end(
                            xr_frame_state.predicted_display_time,
                            xr_context.blend_mode,
                            &[&openxr::CompositionLayerProjection::new()
                                .space(&xr_space)
                                .views(&[
                                    openxr::CompositionLayerProjectionView::new()
                                        .pose(views[0].pose)
                                        .fov(views[0].fov)
                                        .sub_image(
                                            openxr::SwapchainSubImage::new()
                                                .swapchain(xr_swapchain)
                                                .image_array_index(0)
                                                .image_rect(rect),
                                        ),
                                    openxr::CompositionLayerProjectionView::new()
                                        .pose(views[1].pose)
                                        .fov(views[1].fov)
                                        .sub_image(
                                            openxr::SwapchainSubImage::new()
                                                .swapchain(xr_swapchain)
                                                .image_array_index(1)
                                                .image_rect(rect),
                                        ),
                                ])],
                        ) {
                            log::error!("Failed to end frame stream: {}", err);
                        };
                    }

                    //XR Input processing
                    if input_context.is_some() {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Process Inputs");

                        let input_context = input_context
                            .as_mut()
                            .context("Cannot borrow input context as mutable")?;

                        if input_context
                            .process_inputs(
                                &xr_session,
                                &xr_frame_state,
                                &xr_reference_space,
                                &xr_view_space,
                            )
                            .is_ok()
                        {
                            if let Some(new_state) = &input_context.input_state {
                                if new_state.hands_near_head > 0
                                    && new_state.near_start.elapsed().as_secs() > 3
                                {
                                    let should_unlock_horizon = new_state.hands_near_head > 1
                                        || (new_state.hands_near_head == 1
                                            && new_state.count_change.elapsed().as_secs() < 1);

                                    if recenter_request.is_none() {
                                        recenter_request = Some(RecenterRequest {
                                            horizon_locked: !should_unlock_horizon,
                                            delay: 0,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    if let Some(recenter_request) = recenter_request.take() {
                        #[cfg(feature = "profiling")]
                        profiling::scope!("Recenter Scene");

                        if let Err(err) = recenter_scene(
                            &xr_session,
                            &xr_reference_space,
                            &xr_view_space,
                            xr_frame_state.predicted_display_time,
                            recenter_request.horizon_locked,
                            recenter_request.delay,
                            &mut xr_space,
                        ) {
                            log::error!("Failed to recenter scene: {}", err);
                        }
                    }
                } else {
                    #[cfg(feature = "profiling")]
                    profiling::scope!("Session Sleep");

                    // avoid looping too fast
                    std::thread::sleep(std::time::Duration::from_millis(11));
                }
            }
        }

        // Non-XR Input processing
        {
            #[cfg(feature = "profiling")]
            profiling::scope!("Interface input update logic");
            match app_state
                .lock()
                .ok()
                .context("Cannot get lock on app state")?
                .message
                .take()
            {
                Some(AppCommands::Quit) => {
                    log::info!("Qutting app manually...");
                    break;
                }
                Some(AppCommands::Reload) => {
                    current_loader = None;
                }
                Some(AppCommands::Recenter(horizon_locked)) => {
                    recenter_request = Some(RecenterRequest {
                        horizon_locked: *horizon_locked,
                        delay: 0,
                    });
                }
                Some(AppCommands::ToggleSettings(setting)) => match setting {
                    ToggleSetting::SwapEyes => {
                        screen_params.swap_eyes = !screen_params.swap_eyes;
                        screen_invalidated = true;
                    }
                    ToggleSetting::FlipX => {
                        screen_params.flip_x = !screen_params.flip_x;
                        match stereo_mode {
                            StereoMode::Sbs | StereoMode::FullSbs => {
                                screen_params.swap_eyes = !screen_params.swap_eyes;
                            }
                            _ => {}
                        }
                        screen_invalidated = true;
                    }
                    ToggleSetting::FlipY => {
                        screen_params.flip_y = !screen_params.flip_y;
                        match stereo_mode {
                            StereoMode::Tab | StereoMode::FullTab => {
                                screen_params.swap_eyes = !screen_params.swap_eyes;
                            }
                            _ => {}
                        }
                        screen_invalidated = true;
                    }
                    ToggleSetting::AmbientLight => {
                        screen_params.ambient = !screen_params.ambient;
                        screen.change_ambient_mode(screen_params.ambient);
                        screen_invalidated = true;
                    }
                },
                _ => {}
            }
        }

        if let Some(ConfigContext {
            config_notifier: Some(config_receiver),
            ..
        }) = config
        {
            #[cfg(feature = "profiling")]
            profiling::scope!("File Config Watcher");

            if config_receiver.try_recv().is_ok() {
                let config = config
                    .as_mut()
                    .context("Cannot borrow configuration as mutable")?;
                let config_changed = config.update_config().is_ok();

                if config_changed {
                    if let Some(new_params) = config.last_config.clone() {
                        screen_params = new_params;
                        screen.change_scale(screen_params.scale);
                        screen.change_distance(-screen_params.distance);
                        screen.change_ambient_mode(screen_params.ambient);
                        screen_invalidated = true;
                    }
                }
            }
        }

        #[cfg(feature = "profiling")]
        profiling::finish_frame!();
    }

    Ok(())
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn upload_blur_uniforms(
    ambient_texture: &RoundRobinTextureBuffer<Texture2D<Bound>, 3>,
    jitter_frame: &mut u32,
    temporal_blur_params: &mut TemporalBlurParams,
    wgpu_context: &WgpuContext,
    temporal_blur_params_buffer: &wgpu::Buffer,
) {
    log::trace!("Writing temporal blur uniforms");
    let ambient_texture = &ambient_texture.current().texture;
    let resolution = [
        ambient_texture.width() as f32,
        ambient_texture.height() as f32,
    ];
    *jitter_frame = (*jitter_frame + 1) % AMBIENT_BLUR_TEMPORAL_SAMPLES;
    temporal_blur_params.jitter = engine::jitter::get_jitter(*jitter_frame, &resolution);
    temporal_blur_params.resolution = resolution;
    wgpu_context.queue.write_buffer(
        temporal_blur_params_buffer,
        0,
        bytemuck::cast_slice(&[temporal_blur_params.uniform()]),
    );
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn upload_camera_uniforms(
    views: &[openxr::View],
    cameras: &mut [Camera],
    camera_uniform: &mut Vec<CameraUniform>,
    wgpu_context: &WgpuContext,
    camera_buffer: &wgpu::Buffer,
) -> Result<(), anyhow::Error> {
    for (view_idx, view) in views.iter().enumerate() {
        let mut eye = cameras
            .get_mut(view_idx)
            .context("Cannot borrow camera as mutable")?;
        eye.entity.position.x = view.pose.position.x;
        eye.entity.position.y = view.pose.position.y;
        eye.entity.position.z = view.pose.position.z;
        eye.entity.rotation.v.x = view.pose.orientation.x;
        eye.entity.rotation.v.y = view.pose.orientation.y;
        eye.entity.rotation.v.z = view.pose.orientation.z;
        eye.entity.rotation.s = view.pose.orientation.w;
        eye.entity.update_matrices(&[]);
        eye.update_projection_from_tangents(view.fov);
        let camera_uniform = camera_uniform
            .get_mut(view_idx)
            .context("Cannot borrow camera uniform buffer as mutable")?;
        camera_uniform.update_view_proj(eye)?;
    }
    log::trace!("Write views");
    wgpu_context.queue.write_buffer(
        camera_buffer,
        0,
        bytemuck::cast_slice(camera_uniform.as_slice()),
    );
    Ok(())
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn get_ambient_texture(
    screen_texture: &Texture2D<Bound>,
    aspect: f32,
    stereo_mode: &StereoMode,
    wgpu_context: &WgpuContext,
    bind_group_layout: &BindGroupLayout,
) -> anyhow::Result<RoundRobinTextureBuffer<Texture2D<Bound>, 3>> {
    let height_multiplier = match stereo_mode {
        StereoMode::FullTab => 2,
        _ => 1,
    };

    let width_multiplier = match stereo_mode {
        StereoMode::FullSbs => 2,
        _ => 1,
    };
    let wpu_format = SWAPCHAIN_COLOR_FORMAT.try_into()?;
    let buffer = RoundRobinTextureBuffer::new(
        (0..3)
            .map(|idx| {
                screen_texture
                    .as_render_target_with_extent(
                        format!("Ambient Texture {idx}").as_str(),
                        wgpu::Extent3d {
                            width: AMBIENT_BLUR_BASE_RES * width_multiplier,
                            height: (AMBIENT_BLUR_BASE_RES as f32 / aspect) as u32
                                * height_multiplier,
                            depth_or_array_layers: screen_texture.texture.depth_or_array_layers(),
                        },
                        wpu_format,
                        &wgpu_context.device,
                    )
                    .bind_to_context(wgpu_context, bind_group_layout)
            })
            .collect::<Vec<_>>()
            .try_into()
            .ok()
            .context("Cannot create ambient texture buffer")?,
    );

    Ok(buffer)
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn recenter_scene(
    xr_session: &openxr::Session<openxr::Vulkan>,
    xr_reference_space: &openxr::Space,
    xr_view_space: &openxr::Space,
    last_predicted_frame_time: openxr::Time,
    horizon_locked: bool,
    delay: i64,
    xr_space: &mut openxr::Space,
) -> anyhow::Result<()> {
    let mut view_location_pose = xr_view_space
        .locate(
            xr_reference_space,
            openxr::Time::from_nanos(last_predicted_frame_time.as_nanos() - delay),
        )?
        .pose;
    let quaternion =
        cgmath::Quaternion::from(mint::Quaternion::from(view_location_pose.orientation));
    let forward = cgmath::Vector3::new(0.0, 0.0, 1.0);
    let look_dir = quaternion * forward;
    let yaw = cgmath::Rad(look_dir.x.atan2(look_dir.z));
    let clean_orientation = if horizon_locked {
        cgmath::Quaternion::from_angle_y(yaw)
    } else {
        let padj = (look_dir.x * look_dir.x + look_dir.z * look_dir.z).sqrt();
        let pitch = -cgmath::Rad(look_dir.y.atan2(padj));
        cgmath::Quaternion::from_angle_y(yaw) * cgmath::Quaternion::from_angle_x(pitch)
    };
    view_location_pose.orientation = openxr::Quaternionf {
        x: clean_orientation.v.x,
        y: clean_orientation.v.y,
        z: clean_orientation.v.z,
        w: clean_orientation.s,
    };
    *xr_space = xr_session.create_reference_space(ReferenceSpaceType::LOCAL, view_location_pose)?;

    Ok(())
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn check_loader_invalidation(
    current_loader: Option<usize>,
    loaders: &[Box<dyn loaders::Loader>],
    screen_invalidated: &mut bool,
) -> anyhow::Result<()> {
    if let Some(loader) = current_loader {
        if loaders
            .get(loader)
            .context("Error getting loader")?
            .is_invalid()
        {
            log::info!("Reloading app...");
            *screen_invalidated = true;
        }
    } else {
        *screen_invalidated = true;
    }

    Ok(())
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn try_to_load_texture(
    loaders: &mut [Box<dyn loaders::Loader>],
    wgpu_context: &WgpuContext,
    current_loader: Option<usize>,
) -> Option<(Texture2D<Unbound>, f32, Option<StereoMode>, usize)> {
    for (loader_idx, loader) in loaders.iter_mut().enumerate() {
        if current_loader == Some(loader_idx) {
            break;
        }

        let loaded_texture = try_loader(loader, wgpu_context, loader_idx);
        if loaded_texture.is_some() {
            return loaded_texture;
        }
    }
    None
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn try_loader(
    loader: &mut Box<dyn Loader>,
    wgpu_context: &WgpuContext,
    loader_idx: usize,
) -> Option<(Texture2D<Unbound>, f32, Option<StereoMode>, usize)> {
    if let Ok(tex_source) = loader.load(
        &wgpu_context.instance,
        &wgpu_context.device,
        &wgpu_context.queue,
    ) {
        let aspect_ratio_multiplier = tex_source
            .stereo_mode
            .as_ref()
            .map(|stereo_mode| stereo_mode.aspect_ratio_multiplier())
            .unwrap_or(1.0);
        return Some((
            tex_source.texture,
            (tex_source.width as f32 * aspect_ratio_multiplier) / tex_source.height as f32,
            tex_source.stereo_mode,
            loader_idx,
        ));
    }
    None
}

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "full"))]
pub fn main() {
    if let Err(err) = launch() {
        log::error!("VRScreenCap closed unexpectedly with an error: {}", err);
    }
}
