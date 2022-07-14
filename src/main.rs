use ash::{extensions::{khr::{Swapchain, ExternalMemoryWin32}, ext::DebugUtils}, vk, prelude::VkResult};
use loaders::Loader;
use wgpu::{BindGroup, util::DeviceExt};
use wgpu_hal::{api::Vulkan, InstanceError};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

pub mod loaders;
pub mod engine;
use std::{borrow::Cow, ffi::{CStr, CString}, os::raw::c_char, slice, ops::Deref, time::Instant};


#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

const TARGET_FPS: u64 = 60;

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];
            
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex { position: [-1.0, -1.0, 0.0], tex_coords: [0.0, 0.0] },
    Vertex { position: [-1.0, 1.0, 0.0], tex_coords: [0.0, 1.0] },
    Vertex { position: [1.0, 1.0, 0.0], tex_coords: [1.0, 1.0] },
    Vertex { position: [1.0, -1.0, 0.0], tex_coords: [1.0, 0.0] },
];

const INDICES: &[u16] = &[
    0, 2, 3,
    0, 1, 2,
];

pub const TARGET_VULKAN_VERSION: u32 = vk::make_api_version(0, 1, 1, 0);

pub fn get_vulkan_instance_extensions(entry: &ash::Entry) -> Result<Vec<&'static CStr>, InstanceError> {
    let mut flags = wgpu_hal::InstanceFlags::empty();
    if cfg!(debug_assertions) {
        flags |= wgpu_hal::InstanceFlags::VALIDATION;
        flags |= wgpu_hal::InstanceFlags::DEBUG;
    }

    <wgpu_hal::api::Vulkan as wgpu_hal::Api>::Instance::required_extensions(entry, flags)
}

// Hal adapter used to get required device extensions and features
pub fn create_wgpu_instance(
    entry: ash::Entry,
    version: u32,
    instance: ash::Instance
) -> Result<wgpu::Instance, InstanceError> {
    let mut instance_extensions = get_vulkan_instance_extensions(&entry)?;
    instance_extensions.push(ash::extensions::khr::ExternalMemoryWin32::name());

    let mut flags = wgpu_hal::InstanceFlags::empty();
    if cfg!(debug_assertions) {
        flags |= wgpu_hal::InstanceFlags::VALIDATION;
        flags |= wgpu_hal::InstanceFlags::DEBUG;
    };

    Ok(unsafe { wgpu::Instance::from_hal::<Vulkan>(
        <wgpu_hal::api::Vulkan as wgpu_hal::Api>::Instance::from_raw(
            entry,
            instance,
            version,
            0,
            instance_extensions,
            flags,
            false,
            None, // <-- the instance is not destroyed on drop
        )?
    )})
}

pub fn create_vulkan_instance(
    entry: &ash::Entry,
    info: &vk::InstanceCreateInfo,
) -> VkResult<ash::Instance> {
    let mut extensions_ptrs = get_vulkan_instance_extensions(entry).unwrap()
        .iter()
        .map(|x| x.as_ptr())
        .collect::<Vec<_>>();

    extensions_ptrs.extend_from_slice(unsafe {
        slice::from_raw_parts(
            info.pp_enabled_extension_names,
            info.enabled_extension_count as _,
        )
    });

    let layers: Vec<&CStr> = vec![];//vec![CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
    let layers_ptrs = layers.iter().map(|x| x.as_ptr()).collect::<Vec<_>>();

    unsafe {
        entry
            .create_instance(
                &vk::InstanceCreateInfo {
                    enabled_extension_count: extensions_ptrs.len() as _,
                    pp_enabled_extension_names: extensions_ptrs.as_ptr(),
                    enabled_layer_count: layers_ptrs.len() as _,
                    pp_enabled_layer_names: layers_ptrs.as_ptr(),
                    ..*info
                },
                None,
            )
    }
}

pub fn get_vulkan_graphics_device(
    instance: &ash::Instance,
    adapter_index: Option<usize>,
) -> VkResult<vk::PhysicalDevice> {
    let mut physical_devices = unsafe { instance.enumerate_physical_devices()? };

    Ok(physical_devices.remove(adapter_index.unwrap_or(0)))
}

async unsafe fn create_wgpu_from_hal() -> wgpu::Instance {
    let entry = ash::Entry::load().unwrap();
    let raw_instance = create_vulkan_instance(
        &entry,
        &vk::InstanceCreateInfo::builder()
            .application_info(
                &vk::ApplicationInfo::builder().api_version(TARGET_VULKAN_VERSION),
            )
            .build(),
    ).unwrap();
    
    create_wgpu_instance(entry.clone(), TARGET_VULKAN_VERSION, raw_instance).unwrap()
}

async fn run(event_loop: EventLoop<()>, window: Window) {
    let size = window.inner_size();
    
    let instance  = unsafe { create_wgpu_from_hal().await };

    let surface = unsafe { instance.create_surface(&window) };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: wgpu::Features::empty(),
                // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
            },
            None,
        )
        .await
        .expect("Failed to create device");

    //Vertices
    let vertex_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        }
    );
    let num_vertices = VERTICES.len() as u32;

    
    let index_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        }
    );
    let num_indices = INDICES.len() as u32;
        
    //Load loaders
    let mut diffuse_bind_group = None;
    let mut bind_group_layouts = vec!();
    #[cfg(target_os = "windows")]
    {
        let mut cata = loaders::katanga_loader::KatangaLoaderContext::default();
        let texture = cata.load(&instance, &device).unwrap();
        // We don't need to configure the texture view much, so let's
        // let wgpu define it.
        let diffuse_texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        diffuse_bind_group = Some(device.create_bind_group(
            &wgpu::BindGroupDescriptor {
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
            }
        ));

        bind_group_layouts.push(texture_bind_group_layout);
    }

    // Load the shaders from disk
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: bind_group_layouts.iter().collect::<Vec<_>>().as_slice(),
        push_constant_ranges: &[],
    });

    let swapchain_format = surface.get_supported_formats(&adapter)[0];

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[
                Vertex::desc(),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
    };

    surface.configure(&device, &config);

    event_loop.run(move |event, _, control_flow| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter, &shader, &pipeline_layout, &vertex_buffer, &index_buffer, &diffuse_bind_group);
        let start_time = Instant::now();

        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                // Reconfigure the surface with the new size
                config.width = size.width;
                config.height = size.height;
                surface.configure(&device, &config);
                // On macos the window needs to be redrawn manually after resizing
                window.request_redraw();
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            },
            Event::RedrawRequested(_) => {
                let frame = surface
                    .get_current_texture()
                    .expect("Failed to acquire next swap chain texture");
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);

                    if let Some(bind_group) = &diffuse_bind_group {
                        rpass.set_bind_group(0, bind_group, &[]);
                    }

                    rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    rpass.draw_indexed(0..num_indices, 0, 0..1);
                }

                queue.submit(Some(encoder.finish()));
                frame.present();
            },
            _ => {}
        }

        match *control_flow {
            ControlFlow::Exit => (),
            _ => {
                /*
                 * Grab window handle from the display (untested - based on API)
                 */
                window.request_redraw();
                /*
                 * Below logic to attempt hitting TARGET_FPS.
                 * Basically, sleep for the rest of our milliseconds
                 */
                let elapsed_time = Instant::now().duration_since(start_time).as_millis() as u64;
    
                let wait_millis = match 1000 / TARGET_FPS >= elapsed_time {
                    true => 1000 / TARGET_FPS - elapsed_time,
                    false => 0
                };
                let new_inst = start_time + std::time::Duration::from_millis(wait_millis);
                *control_flow = ControlFlow::WaitUntil(new_inst);
            }
        }
    });
}

fn main() {
    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        // Temporarily avoid srgb formats for the swapchain on the web
        pollster::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("could not initialize logger");
        use winit::platform::web::WindowExtWebSys;
        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");
        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}