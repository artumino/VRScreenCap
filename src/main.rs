use ash::vk::{self, Handle};
use engine::{
    geometry::Vertex,
    vr::{enable_xr_runtime, OpenXRContext},
    WgpuLoader,
};
use openxr::sys::create_swapchain;
use std::{borrow::Cow, convert::TryInto, num::NonZeroU32, sync::Arc};
use wgpu::{
    Adapter, ColorTargetState, Device, Extent3d, Instance, Queue, ShaderSource, TextureAspect,
    TextureDescriptor, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

use crate::{engine::geometry::Mesh, loaders::Loader};

mod conversions;
mod engine;
mod loaders;

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let mut xr_context = enable_xr_runtime().unwrap();
    let wgpu_context = xr_context.load_wgpu().unwrap();

    // Load the shaders from disk
    let shader = wgpu_context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

    // We don't need to configure the texture view much, so let's
    // let wgpu define it.

    let mut bind_group_layouts = vec![];

    let mut screen_texture = wgpu_context.device.create_texture(&TextureDescriptor {
        label: "Blank".into(),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
    });

    let mut aspect_ratio = 1.0;
    #[cfg(target_os = "windows")]
    {
        let mut cata = loaders::katanga_loader::KatangaLoaderContext::default();
        if let Ok(tex_source) = cata.load(&wgpu_context.instance, &wgpu_context.device) {
            screen_texture = tex_source.texture;
            aspect_ratio = (tex_source.width as f32/2.0) / tex_source.height as f32;
        }
    }

    let screen = Mesh::get_rectangle(aspect_ratio, 1.0);
    let (screen_vertex_buffer, screen_index_buffer) = screen.get_buffers(&wgpu_context.device);

    let diffuse_texture_view = screen_texture.create_view(&wgpu::TextureViewDescriptor::default());
    let diffuse_sampler = wgpu_context
        .device
        .create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
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

    let diffuse_bind_group = wgpu_context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                }
            ],
            label: Some("diffuse_bind_group"),
        });
    bind_group_layouts.push(texture_bind_group_layout);

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
                    targets: &[Some(TextureFormat::Bgra8UnormSrgb.into())],
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
    let stage = xr_session
        .create_reference_space(openxr::ReferenceSpaceType::STAGE, openxr::Posef::IDENTITY)
        .unwrap();

    let mut event_storage = openxr::EventDataBuffer::new();
    let mut session_running = false;
    let mut swapchain = None;
    // Handle OpenXR events
    loop {
        let event = xr_context.instance.poll_event(&mut event_storage).unwrap();
        match event {
            Some(openxr::Event::SessionStateChanged(e)) => {
                // Session state change is where we can begin and end sessions, as well as
                // find quit messages!
                println!("Entered state {:?}", e.state());
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
                println!("Lost {} OpenXR events", e.lost_event_count());
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
                        return;
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
                                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                    store: true,
                                },
                            })],
                            depth_stencil_attachment: None,
                        });
                        rpass.set_pipeline(&render_pipeline);

                        rpass.set_bind_group(0, &diffuse_bind_group, &[]);
                        rpass.set_vertex_buffer(0, screen_vertex_buffer.slice(..));
                        rpass.set_index_buffer(
                            screen_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                        rpass.draw_indexed(0..screen.indices(), 0, 0..1);
                    }

                    // Fetch the view transforms. To minimize latency, we intentionally do this
                    // *after* recording commands to render the scene, i.e. at the last possible
                    // moment before rendering begins in earnest on the GPU. Uniforms dependent on
                    // this data can be sent to the GPU just-in-time by writing them to per-frame
                    // host-visible memory which the GPU will only read once the command buffer is
                    // submitted.
                    let (_, views) = xr_session
                        .locate_views(VIEW_TYPE, xr_frame_state.predicted_display_time, &stage)
                        .unwrap();

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
                                .space(&stage)
                                .views(&[
                                    openxr::CompositionLayerProjectionView::new()
                                        .pose(views[0].pose)
                                        .fov(views[0].fov)
                                        .sub_image(
                                            openxr::SwapchainSubImage::new()
                                                .swapchain(&xr_swapchain)
                                                .image_array_index(0)
                                                .image_rect(rect),
                                        ),
                                    openxr::CompositionLayerProjectionView::new()
                                        .pose(views[1].pose)
                                        .fov(views[1].fov)
                                        .sub_image(
                                            openxr::SwapchainSubImage::new()
                                                .swapchain(&xr_swapchain)
                                                .image_array_index(1)
                                                .image_rect(rect),
                                        ),
                                ])],
                        )
                        .unwrap();
                }
            }
        }
    }
}

const VIEW_TYPE: openxr::ViewConfigurationType = openxr::ViewConfigurationType::PRIMARY_STEREO;
