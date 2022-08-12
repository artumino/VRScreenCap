#![windows_subsystem = "windows"]
use ::windows::Win32::System::Threading::{
    GetCurrentProcess, SetPriorityClass, HIGH_PRIORITY_CLASS,
};
use ash::vk::Handle;
use clap::Parser;
use engine::{
    camera::{Camera, CameraUniform},
    geometry::Vertex,
    vr::{enable_xr_runtime, OpenXRContext},
    WgpuContext, WgpuLoader, screen::Screen,
};
use loaders::StereoMode;
use config::AppConfig;
#[cfg(not(debug_assertions))]
use log::LevelFilter;
#[cfg(not(debug_assertions))]
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use std::{
    borrow::Cow,
    num::NonZeroU32,
    sync::{Arc, Mutex},
};
use thread_priority::*;
use tray_item::TrayItem;
use wgpu::{util::DeviceExt, ShaderSource, TextureDescriptor, TextureFormat};

use crate::config::ConfigContext;

mod conversions;
mod engine;
mod loaders;
mod config;

enum TrayMessages {
    Quit,
    Reload,
    ToggleSettings(ToggleSetting),
}

enum ToggleSetting {
    FlipX,
    FlipY,
    SwapEyes,
}

struct TrayState {
    pub message: Option<TrayMessages>,
}

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    #[cfg(feature = "renderdoc")]
    let _rd: renderdoc::RenderDoc<renderdoc::V110> =
        renderdoc::RenderDoc::new().expect("Unable to connect");

    #[cfg(not(debug_assertions))]
    {
        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
            .build("output.log")
            .unwrap();

        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Info))
            .unwrap();

        log4rs::init_config(config).unwrap();
        log_panics::init();
    }

    try_elevate_priority();

    let mut xr_context = enable_xr_runtime().unwrap();
    let wgpu_context = xr_context.load_wgpu().unwrap();
    let (_tray, tray_state) = build_tray();
    let mut config_context = config::ConfigContext::try_setup().unwrap_or(None);

    log::info!("Finished initial setup, running main loop");
    run(&mut xr_context, &wgpu_context, &tray_state, &mut config_context);
}

fn build_tray() -> (TrayItem, Arc<Mutex<TrayState>>) {
    log::info!("Building system tray");
    let mut tray = TrayItem::new("VR Screen Cap", "tray-icon").unwrap();
    let tray_state = Arc::new(Mutex::new(TrayState { message: None }));

    let cloned_state = tray_state.clone();
    tray.add_menu_item("Quit", move || {
        cloned_state.lock().unwrap().message = Some(TrayMessages::Quit);
    })
    .unwrap();

    tray.add_label("Settings").unwrap();

    let cloned_state = tray_state.clone();
    tray.add_menu_item("Swap Eyes", move || {
        cloned_state.lock().unwrap().message =
            Some(TrayMessages::ToggleSettings(ToggleSetting::SwapEyes));
    })
    .unwrap();

    let cloned_state = tray_state.clone();
    tray.add_menu_item("Flip X", move || {
        cloned_state.lock().unwrap().message =
            Some(TrayMessages::ToggleSettings(ToggleSetting::FlipX));
    })
    .unwrap();

    let cloned_state = tray_state.clone();
    tray.add_menu_item("Flip Y", move || {
        cloned_state.lock().unwrap().message =
            Some(TrayMessages::ToggleSettings(ToggleSetting::FlipY));
    })
    .unwrap();

    let cloned_state = tray_state.clone();
    tray.add_menu_item("Reload Screen", move || {
        cloned_state.lock().unwrap().message = Some(TrayMessages::Reload);
    })
    .unwrap();

    (tray, tray_state)
}

fn try_elevate_priority() {
    log::info!("Trying to elevate process priority");
    if set_current_thread_priority(ThreadPriority::Os(WinAPIThreadPriority::Highest.into()))
        .is_err()
    {
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
) {
    // Load the shaders from disk
    let shader = wgpu_context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

    // We don't need to configure the texture view much, so let's
    // let wgpu define it.

    let mut aspect_ratio = 1.0;
    let mut stereo_mode = StereoMode::Mono;
    let mut current_loader = None;

    //Load blank texture
    let blank_data = image::load_from_memory(include_bytes!("../assets/blank_grey.png"))
        .unwrap()
        .to_rgba8();

    let blank_size = wgpu::Extent3d {
        width: blank_data.dimensions().0,
        height: blank_data.dimensions().1,
        depth_or_array_layers: 1,
    };

    let mut screen_texture = wgpu_context.device.create_texture(&TextureDescriptor {
        label: "Blank".into(),
        size: blank_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    });
    wgpu_context.queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &screen_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &blank_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: std::num::NonZeroU32::new(blank_size.width * 4),
            rows_per_image: std::num::NonZeroU32::new(blank_size.height),
        },
        blank_size,
    );

    let mut loaders = vec![
        #[cfg(target_os = "windows")]
        {
            loaders::katanga_loader::KatangaLoaderContext::default()
        },
    ];

    if let Some((texture, aspect, mode, loader)) = try_to_load_texture(&mut loaders, wgpu_context) {
        screen_texture = texture;
        aspect_ratio = aspect;
        stereo_mode = mode;
        current_loader = Some(loader);
    }

    let mut screen_params = match config {
        Some(ConfigContext{
            last_config: Some(config),
            ..}) => config.clone(),
        _ => AppConfig::parse()
    };
    let mut screen = Screen::new(
        -screen_params.distance,
        screen_params.scale,
        aspect_ratio,
    );
    let (mut screen_vertex_buffer, mut screen_index_buffer) =
        screen.mesh.get_buffers(&wgpu_context.device);

    let screen_params_buffer =
        wgpu_context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Screen Params Buffer"),
                contents: bytemuck::cast_slice(&[screen_params.uniform()]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

    let screen_model_matrix_buffer = wgpu_context
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

    let diffuse_sampler = wgpu_context
        .device
        .create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

    let bind_group_layouts = vec![
        texture_bind_group_layout,
        global_uniform_bind_group_layout
    ];

    // Not pretty at all, but it works.
    let texture_bind_group_layout = bind_group_layouts.get(0).unwrap();
    let mut diffuse_bind_group = bind_texture(
        wgpu_context,
        texture_bind_group_layout,
        &screen_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        &diffuse_sampler,
    );

    let pipeline_layout =
        wgpu_context
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: bind_group_layouts.iter().collect::<Vec<_>>().as_slice(),
                push_constant_ranges: &[],
            });

    let render_pipeline =
        wgpu_context
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(TextureFormat::Bgra8Unorm.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: NonZeroU32::new(2),
            });

    // Start the OpenXR session
    // TODO: Use hal methods for wgpu
    let (xr_session, mut frame_wait, mut frame_stream) = unsafe {
        xr_context
            .instance
            .create_session::<openxr::Vulkan>(
                xr_context.system,
                &openxr::vulkan::SessionCreateInfo {
                    instance: wgpu_context.vk_instance.handle().as_raw() as _,
                    physical_device: wgpu_context.vk_phys_device.as_raw() as _,
                    device: wgpu_context.vk_device.handle().as_raw() as _,
                    queue_family_index: wgpu_context.queue_index,
                    queue_index: 0,
                },
            )
            .unwrap()
    };

    // Create a room-scale reference space
    let xr_space = xr_session
        .create_reference_space(openxr::ReferenceSpaceType::LOCAL, openxr::Posef::IDENTITY)
        .unwrap();

    let mut event_storage = openxr::EventDataBuffer::new();
    let mut session_running = false;
    let mut swapchain = None;
    let mut screen_invalidated = false;
    let mut last_invalidation_check = std::time::Instant::now();

    // Handle OpenXR events
    loop {
        let time = std::time::Instant::now();

        if current_loader.is_some() || time.duration_since(last_invalidation_check).as_secs() > 10 {
            check_loader_invalidation(current_loader, &loaders, &mut screen_invalidated);
            last_invalidation_check = time;
        }

        if screen_invalidated {
            if let Some((texture, aspect, mode, loader)) =
                try_to_load_texture(&mut loaders, wgpu_context)
            {
                screen_texture.destroy();
                screen_texture = texture;
                aspect_ratio = aspect;
                stereo_mode = mode;
                current_loader = Some(loader);
                screen.change_aspect_ratio(aspect_ratio);
                diffuse_bind_group = bind_texture(
                    wgpu_context,
                    texture_bind_group_layout,
                    &screen_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    &diffuse_sampler,
                );
                (screen_vertex_buffer, screen_index_buffer) =
                    screen.mesh.get_buffers(&wgpu_context.device);

                
                wgpu_context.queue.write_buffer(
                    &screen_model_matrix_buffer,
                    0,
                    bytemuck::cast_slice(&[screen.entity.uniform()]),
                )
            }
            screen_invalidated = false;
        }

        let event = xr_context.instance.poll_event(&mut event_storage).unwrap();
        match event {
            Some(openxr::Event::SessionStateChanged(e)) => {
                // Session state change is where we can begin and end sessions, as well as
                // find quit messages!
                log::info!("Entered state {:?}", e.state());
                match e.state() {
                    openxr::SessionState::READY => {
                        xr_session.begin(VIEW_TYPE).unwrap();
                        session_running = true;
                    }
                    openxr::SessionState::STOPPING => {
                        xr_session.end().unwrap();
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
            _ => {
                // Render to HMD only if we have an active session
                if session_running {
                    // Block until the previous frame is finished displaying, and is ready for
                    // another one. Also returns a prediction of when the next frame will be
                    // displayed, for use with predicting locations of controllers, viewpoints, etc.
                    let xr_frame_state = frame_wait.wait().unwrap();

                    // Must be called before any rendering is done!
                    frame_stream.begin().unwrap();

                    // Only render if we should
                    if !xr_frame_state.should_render {
                        // Early bail
                        frame_stream
                            .end(
                                xr_frame_state.predicted_display_time,
                                xr_context.blend_mode,
                                &[],
                            )
                            .unwrap();
                        continue;
                    }

                    // If we do not have a swapchain yet, create it
                    let (xr_swapchain, resolution, image_views) =
                        swapchain.get_or_insert_with(|| {
                            xr_context.create_swapchain(&xr_session, &wgpu_context.device)
                        });

                    // Check which image we need to render to and wait until the compositor is
                    // done with this image
                    let image_index = xr_swapchain.acquire_image().unwrap();
                    xr_swapchain.wait_image(openxr::Duration::INFINITE).unwrap();
                    let muti_view = &image_views[image_index as usize];

                    // Render!
                    let mut encoder = wgpu_context
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: muti_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: true,
                                },
                            })],
                            depth_stencil_attachment: None,
                        });
                        rpass.set_pipeline(&render_pipeline);

                        rpass.set_bind_group(0, &diffuse_bind_group, &[]);
                        rpass.set_bind_group(1, &global_uniform_bind_group, &[]);
                        rpass.set_vertex_buffer(0, screen_vertex_buffer.slice(..));
                        rpass.set_index_buffer(
                            screen_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        rpass.draw_indexed(0..screen.mesh.indices(), 0, 0..1);
                    }

                    // Fetch the view transforms. To minimize latency, we intentionally do this
                    // *after* recording commands to render the scene, i.e. at the last possible
                    // moment before rendering begins in earnest on the GPU. Uniforms dependent on
                    // this data can be sent to the GPU just-in-time by writing them to per-frame
                    // host-visible memory which the GPU will only read once the command buffer is
                    // submitted.
                    let (_, views) = xr_session
                        .locate_views(VIEW_TYPE, xr_frame_state.predicted_display_time, &xr_space)
                        .unwrap();

                    for (view_idx, view) in views.iter().enumerate() {
                        let mut eye = cameras.get_mut(view_idx).unwrap();
                        eye.entity.position.x = view.pose.position.x;
                        eye.entity.position.y = view.pose.position.y;
                        eye.entity.position.z = view.pose.position.z;
                        eye.entity.rotation.v.x = view.pose.orientation.x;
                        eye.entity.rotation.v.y = view.pose.orientation.y;
                        eye.entity.rotation.v.z = view.pose.orientation.z;
                        eye.entity.rotation.s = view.pose.orientation.w;
                        eye.entity.update_matrices(&[]);
                        eye.update_projection_from_tangents(view.fov);
                        let camera_uniform = camera_uniform.get_mut(view_idx).unwrap();
                        camera_uniform.update_view_proj(eye);
                    }

                    wgpu_context.queue.write_buffer(
                        &camera_buffer,
                        0,
                        bytemuck::cast_slice(camera_uniform.as_slice()),
                    );
                    wgpu_context.queue.submit(Some(encoder.finish()));
                    xr_swapchain.release_image().unwrap();

                    // End rendering and submit the images
                    let rect = openxr::Rect2Di {
                        offset: openxr::Offset2Di { x: 0, y: 0 },
                        extent: openxr::Extent2Di {
                            width: resolution.width as _,
                            height: resolution.height as _,
                        },
                    };
                    frame_stream
                        .end(
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
                        )
                        .unwrap();
                }
            }
        }

        match tray_state.lock().unwrap().message.take() {
            Some(TrayMessages::Quit) => {
                log::info!("Qutting app manually...");
                return;
            }
            Some(TrayMessages::Reload) => {
                check_loader_invalidation(current_loader, &loaders, &mut screen_invalidated);
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

        if let Some(ConfigContext{
            config_notifier: Some(config_receiver),
            ..
        }) = config {
            if config_receiver.try_recv().is_ok() {
                let config = config.as_mut().unwrap();
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
}

fn check_loader_invalidation(
    current_loader: Option<usize>,
    loaders: &[impl loaders::Loader],
    screen_invalidated: &mut bool,
) {
    if let Some(loader) = current_loader {
        if loaders.get(loader).unwrap().is_invalid() {
            log::info!("Reloading app...");
            *screen_invalidated = true;
        }
    } else {
        *screen_invalidated = true;
    }
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
    loaders: &mut [impl loaders::Loader],
    wgpu_context: &WgpuContext,
) -> Option<(wgpu::Texture, f32, StereoMode, usize)> {
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

const VIEW_TYPE: openxr::ViewConfigurationType = openxr::ViewConfigurationType::PRIMARY_STEREO;
