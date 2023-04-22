#[cfg(target_os = "windows")]
use ::windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS,
};
use anyhow::Context;
use cgmath::Rotation3;
use clap::Parser;
use config::AppConfig;
use engine::{
    camera::{Camera, CameraUniform},
    geometry::{ModelVertex, Vertex},
    input::InputContext,
    screen::Screen,
    texture::Texture2D,
    vr::{enable_xr_runtime, OpenXRContext, SWAPCHAIN_COLOR_FORMAT, VIEW_COUNT, VIEW_TYPE},
    WgpuContext, WgpuLoader,
};
use loaders::{katanga_loader::KatangaLoaderContext, Loader, StereoMode};
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use openxr::ReferenceSpaceType;
use std::{
    iter,
    num::NonZeroU32,
    sync::{Arc, Mutex},
};
use thread_priority::*;
#[cfg(not(target_os = "android"))]
use tray_item::TrayItem;
use wgpu::util::DeviceExt;

use crate::config::ConfigContext;

mod config;
mod conversions;
mod engine;
mod loaders;

#[derive(Clone)]
enum TrayMessages {
    Quit,
    Reload,
    Recenter(bool),
    ToggleSettings(ToggleSetting),
}

#[derive(Clone)]
enum ToggleSetting {
    FlipX,
    FlipY,
    SwapEyes,
}

struct TrayState {
    pub message: Option<&'static TrayMessages>,
}

struct RecenterRequest {
    pub delay: i64,
    pub horizon_locked: bool,
}

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
    {
        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
            .build("output.log")?;

        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

        log4rs::init_config(config)?;
        log_panics::init();
    }

    try_elevate_priority();

    let mut xr_context = enable_xr_runtime()?;
    let wgpu_context = xr_context.load_wgpu()?;

    #[cfg(not(target_os = "android"))]
    let (_tray, tray_state) = build_tray()?;
    #[cfg(target_os = "android")]
    let tray_state = Arc::new(Mutex::new(TrayState { message: None }));

    let mut config_context = config::ConfigContext::try_setup().unwrap_or(None);

    log::info!("Finished initial setup, running main loop");
    run(
        &mut xr_context,
        &wgpu_context,
        &tray_state,
        &mut config_context,
    )?;

    Ok(())
}

#[cfg(not(target_os = "android"))]
fn add_tray_message_sender(
    tray_state: &Arc<Mutex<TrayState>>,
    tray: &mut TrayItem,
    entry_name: &'static str,
    message: &'static TrayMessages,
) -> anyhow::Result<()> {
    let cloned_state = tray_state.clone();
    Ok(tray.add_menu_item(entry_name, move || {
        if let Ok(mut locked_state) = cloned_state.lock() {
            locked_state.message = Some(message);
        }
    })?)
}

#[cfg(not(target_os = "android"))]
fn add_all_tray_message_senders(
    tray_state: &Arc<Mutex<TrayState>>,
    tray: &mut TrayItem,
    entries: Vec<(&'static str, &'static TrayMessages)>,
) -> anyhow::Result<()> {
    for (entry_name, message) in entries {
        add_tray_message_sender(tray_state, tray, entry_name, message)?;
    }
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn build_tray() -> anyhow::Result<(TrayItem, Arc<Mutex<TrayState>>)> {
    log::info!("Building system tray");
    let mut tray = TrayItem::new("VR Screen Cap", "tray-icon")?;
    let tray_state = Arc::new(Mutex::new(TrayState { message: None }));

    tray.add_label("Settings")?;
    add_all_tray_message_senders(
        &tray_state,
        &mut tray,
        vec![
            (
                "Swap Eyes",
                &TrayMessages::ToggleSettings(ToggleSetting::SwapEyes),
            ),
            (
                "Flip X",
                &TrayMessages::ToggleSettings(ToggleSetting::FlipX),
            ),
            (
                "Flip Y",
                &TrayMessages::ToggleSettings(ToggleSetting::FlipY),
            ),
        ],
    )?;

    tray.add_label("Actions")?;
    add_all_tray_message_senders(
        &tray_state,
        &mut tray,
        vec![
            ("Reload Screen", &TrayMessages::Reload),
            ("Recenter", &TrayMessages::Recenter(true)),
            ("Recenter w/ Pitch", &TrayMessages::Recenter(false)),
            ("Quit", &TrayMessages::Quit),
        ],
    )?;

    Ok((tray, tray_state))
}

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

fn run(
    xr_context: &mut OpenXRContext,
    wgpu_context: &WgpuContext,
    tray_state: &Arc<Mutex<TrayState>>,
    config: &mut Option<ConfigContext>,
) -> anyhow::Result<()> {
    // Load the shaders from disk
    let shader = wgpu_context
        .device
        .create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

    // We don't need to configure the texture view much, so let's
    // let wgpu define it.

    let mut aspect_ratio = 1.0;
    let mut stereo_mode = StereoMode::Mono;
    let mut current_loader = None;

    //Load blank texture
    let blank_texture = Texture2D::from_bytes(
        &wgpu_context.device,
        &wgpu_context.queue,
        include_bytes!("../assets/blank_grey.png"),
        "Blank",
    )?;

    let mut loaders: Vec<Box<dyn Loader>> = vec![
        #[cfg(target_os = "windows")]
        {
            Box::<KatangaLoaderContext>::default()
        },
    ];

    let mut screen_texture = blank_texture;
    if let Some((texture, aspect, mode, loader)) = try_to_load_texture(&mut loaders, wgpu_context) {
        screen_texture = texture;
        aspect_ratio = aspect;
        stereo_mode = mode;
        current_loader = Some(loader);
    }

    let mut screen_params = match config {
        Some(ConfigContext {
            last_config: Some(config),
            ..
        }) => config.clone(),
        _ => AppConfig::parse(),
    };
    let mut screen = Screen::new(
        &wgpu_context.device,
        -screen_params.distance,
        screen_params.scale,
        aspect_ratio,
        true,
    );

    let screen_params_buffer =
        wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Screen Params Buffer"),
                contents: bytemuck::cast_slice(&[screen_params.uniform()]),
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

    let bind_group_layouts = vec![texture_bind_group_layout, global_uniform_bind_group_layout];

    // Not pretty at all, but it works.
    let texture_bind_group_layout = bind_group_layouts
        .get(0)
        .context("Failed to get texture bind group layout")?;
    let mut diffuse_bind_group = bind_texture(
        wgpu_context,
        texture_bind_group_layout,
        &screen_texture.view,
        &screen_texture.sampler,
    );

    let pipeline_layout =
        wgpu_context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: bind_group_layouts.iter().collect::<Vec<_>>().as_slice(),
                push_constant_ranges: &[],
            });

    let render_pipeline =
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[ModelVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SWAPCHAIN_COLOR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: NonZeroU32::new(VIEW_COUNT),
            });

    let blur_pipeline =
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Blur Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "blur_vs_main",
                    buffers: &[ModelVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "blur_fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: SWAPCHAIN_COLOR_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: NonZeroU32::new(VIEW_COUNT),
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
    // Handle OpenXR events
    loop {
        #[cfg(feature = "profiling")]
        profiling::scope!("main loop");

        let time = std::time::Instant::now();

        if current_loader.is_some() || time.duration_since(last_invalidation_check).as_secs() > 10 {
            check_loader_invalidation(current_loader, &loaders, &mut screen_invalidated)?;
            last_invalidation_check = time;
        }

        if screen_invalidated {
            if let Some((texture, aspect, mode, loader)) =
                try_to_load_texture(&mut loaders, wgpu_context)
            {
                screen_texture = texture;
                aspect_ratio = aspect;
                stereo_mode = mode;
                current_loader = Some(loader);
                screen.change_aspect_ratio(aspect_ratio);
                diffuse_bind_group = bind_texture(
                    wgpu_context,
                    texture_bind_group_layout,
                    &screen_texture.view,
                    &screen_texture.sampler,
                );

                wgpu_context.queue.write_buffer(
                    &screen_model_matrix_buffer,
                    0,
                    bytemuck::cast_slice(&[screen.entity.uniform()]),
                )
            }
            screen_invalidated = false;
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
                    #[cfg(feature = "profiling")]
                    profiling::scope!("Wait for frame");
                    let xr_frame_state = frame_wait.wait()?;

                    // Must be called before any rendering is done!
                    frame_stream.begin()?;

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
                    profiling::scope!("Swapchain Setup");

                    // If we do not have a swapchain yet, create it
                    let (xr_swapchain, resolution, swapchain_textures) = match swapchain {
                        Some(ref mut swapchain) => swapchain,
                        None => {
                            let new_swapchain =
                                xr_context.create_swapchain(&xr_session, &wgpu_context.device)?;
                            swapchain.get_or_insert(new_swapchain)
                        }
                    };

                    // Check which image we need to render to and wait until the compositor is
                    // done with this image
                    let image_index = xr_swapchain.acquire_image()?;
                    xr_swapchain.wait_image(openxr::Duration::INFINITE)?;

                    let swapchain_view = &swapchain_textures[image_index as usize].view;

                    log::trace!("Encode render pass");
                    #[cfg(feature = "profiling")]
                    profiling::scope!("Encode Render Pass");
                    // Render!
                    let mut encoder = wgpu_context.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encorder"),
                        },
                    );
                    {
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
                        if let (Some(ref ambient_mesh),) = (&screen.ambient_mesh,) {
                            rpass.set_pipeline(&blur_pipeline);

                            rpass.set_bind_group(0, &diffuse_bind_group, &[]);
                            rpass.set_bind_group(1, &global_uniform_bind_group, &[]);
                            rpass.set_vertex_buffer(0, ambient_mesh.vertex_buffer().slice(..));
                            rpass.set_index_buffer(
                                ambient_mesh.index_buffer().slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            rpass.draw_indexed(0..ambient_mesh.indices(), 0, 0..1);
                        }

                        // Render the screen
                        rpass.set_pipeline(&render_pipeline);
                        rpass.set_bind_group(0, &diffuse_bind_group, &[]);
                        rpass.set_bind_group(1, &global_uniform_bind_group, &[]);
                        rpass.set_vertex_buffer(0, screen.mesh.vertex_buffer().slice(..));
                        rpass.set_index_buffer(
                            screen.mesh.index_buffer().slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        rpass.draw_indexed(0..screen.mesh.indices(), 0, 0..1);
                    }

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Locate Views");
                    log::trace!("Locate views");
                    // Fetch the view transforms. To minimize latency, we intentionally do this
                    // *after* recording commands to render the scene, i.e. at the last possible
                    // moment before rendering begins in earnest on the GPU. Uniforms dependent on
                    // this data can be sent to the GPU just-in-time by writing them to per-frame
                    // host-visible memory which the GPU will only read once the command buffer is
                    // submitted.
                    let (_, views) = xr_session.locate_views(
                        VIEW_TYPE,
                        xr_frame_state.predicted_display_time,
                        &xr_space,
                    )?;

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
                        &camera_buffer,
                        0,
                        bytemuck::cast_slice(camera_uniform.as_slice()),
                    );

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Encode Submit");

                    log::trace!("Submit command buffer");
                    wgpu_context.queue.submit(iter::once(encoder.finish()));

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Release Swapchain");
                    log::trace!("Release swapchain image");
                    xr_swapchain.release_image()?;

                    // End rendering and submit the images
                    let rect = openxr::Rect2Di {
                        offset: openxr::Offset2Di { x: 0, y: 0 },
                        extent: openxr::Extent2Di {
                            width: resolution.width as _,
                            height: resolution.height as _,
                        },
                    };

                    log::trace!("End frame stream");

                    #[cfg(feature = "profiling")]
                    {
                        let predicted_display_time_nanos =
                            xr_frame_state.predicted_display_time.as_nanos();
                        profiling::scope!(
                            "Show Time Calculation",
                            format!("{predicted_display_time_nanos}").as_str()
                        );
                    }
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
                }

                #[cfg(feature = "profiling")]
                profiling::finish_frame!();
            }
        }

        // Non-XR Input processing
        match tray_state
            .lock()
            .ok()
            .context("Cannot get lock on icon tray state")?
            .message
            .take()
        {
            Some(TrayMessages::Quit) => {
                log::info!("Qutting app manually...");
                break;
            }
            Some(TrayMessages::Reload) => {
                check_loader_invalidation(current_loader, &loaders, &mut screen_invalidated)?;
            }
            Some(TrayMessages::Recenter(horizon_locked)) => {
                recenter_request = Some(RecenterRequest {
                    horizon_locked: *horizon_locked,
                    delay: 0,
                });
            }
            Some(TrayMessages::ToggleSettings(setting)) => match setting {
                ToggleSetting::SwapEyes => {
                    screen_params.swap_eyes = !screen_params.swap_eyes;
                    wgpu_context.queue.write_buffer(
                        &screen_params_buffer,
                        0,
                        bytemuck::cast_slice(&[screen_params.uniform()]),
                    )
                }
                ToggleSetting::FlipX => {
                    screen_params.flip_x = !screen_params.flip_x;
                    match stereo_mode {
                        StereoMode::Sbs | StereoMode::FullSbs => {
                            screen_params.swap_eyes = !screen_params.swap_eyes;
                        }
                        _ => {}
                    }
                    wgpu_context.queue.write_buffer(
                        &screen_params_buffer,
                        0,
                        bytemuck::cast_slice(&[screen_params.uniform()]),
                    )
                }
                ToggleSetting::FlipY => {
                    screen_params.flip_y = !screen_params.flip_y;
                    match stereo_mode {
                        StereoMode::Tab | StereoMode::FullTab => {
                            screen_params.swap_eyes = !screen_params.swap_eyes;
                        }
                        _ => {}
                    }
                    wgpu_context.queue.write_buffer(
                        &screen_params_buffer,
                        0,
                        bytemuck::cast_slice(&[screen_params.uniform()]),
                    )
                }
            },
            _ => {}
        }

        if let Some(ConfigContext {
            config_notifier: Some(config_receiver),
            ..
        }) = config
        {
            if config_receiver.try_recv().is_ok() {
                let config = config
                    .as_mut()
                    .context("Cannot borrow configuration as mutable")?;
                let config_changed = config.update_config().is_ok();

                if config_changed {
                    if let Some(new_params) = config.last_config.clone() {
                        screen_params = new_params;
                        wgpu_context.queue.write_buffer(
                            &screen_params_buffer,
                            0,
                            bytemuck::cast_slice(&[screen_params.uniform()]),
                        );

                        screen.change_scale(screen_params.scale);
                        screen.change_distance(-screen_params.distance);
                        wgpu_context.queue.write_buffer(
                            &screen_model_matrix_buffer,
                            0,
                            bytemuck::cast_slice(&[screen.entity.uniform()]),
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

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

fn bind_texture(
    wgpu_context: &WgpuContext,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    diffuse_texture_view: &wgpu::TextureView,
    diffuse_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    wgpu_context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            layout: texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(diffuse_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(diffuse_sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        })
}

fn try_to_load_texture(
    loaders: &mut [Box<dyn loaders::Loader>],
    wgpu_context: &WgpuContext,
) -> Option<(Texture2D, f32, StereoMode, usize)> {
    for (loader_idx, loader) in loaders.iter_mut().enumerate() {
        if let Ok(tex_source) = loader.load(&wgpu_context.instance, &wgpu_context.device) {
            return Some((
                tex_source.texture,
                (tex_source.width as f32 / 2.0) / tex_source.height as f32,
                tex_source.stereo_mode,
                loader_idx,
            ));
        }
    }
    None
}

#[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "full"))]
pub fn main() {
    if let Err(err) = launch() {
        log::error!("VRScreenCap closed unexpectedly with an error: {}", err);
    }
}
