use std::error::Error;

use ash::vk::{self, Handle};
use hal::MemoryFlags;
use openxr as xr;
use wgpu::{Device, Extent3d, Texture};
use wgpu_hal as hal;

use crate::conversions::vulkan_image_to_texture;

use super::{WgpuLoader, WgpuRunner, TARGET_VULKAN_VERSION};

pub struct OpenXRContext {
    pub entry: openxr::Entry,
    pub instance: openxr::Instance,
    pub props: openxr::InstanceProperties,
    pub system: openxr::SystemId,
    pub blend_mode: openxr::EnvironmentBlendMode,
}

pub const VIEW_TYPE: openxr::ViewConfigurationType = openxr::ViewConfigurationType::PRIMARY_STEREO;
pub const VIEW_COUNT: u32 = 2;

pub fn enable_xr_runtime() -> Result<OpenXRContext, Box<dyn Error>> {
    #[cfg(not(target_os = "android"))]
    let entry = openxr::Entry::linked();
    #[cfg(target_os = "android")]
    let entry = unsafe { openxr::Entry::load()? };

    #[cfg(target_os = "android")]
    entry.initialize_android_loader().unwrap();

    let available_extensions = entry.enumerate_extensions().unwrap();
    log::info!("Available extensions: {:?}", available_extensions);
    assert!(available_extensions.khr_vulkan_enable2);

    let mut enabled_extensions = openxr::ExtensionSet::default();
    enabled_extensions.khr_vulkan_enable2 = true;

    #[cfg(target_os = "android")]
    {
        enabled_extensions.khr_android_create_instance = true;
    }
    log::info!("Enabled extensions: {:?}", enabled_extensions);

    let instance = entry.create_instance(
        &openxr::ApplicationInfo {
            application_name: "VR Screen Viewer",
            application_version: 0,
            engine_name: "void*",
            engine_version: 0,
        },
        &enabled_extensions,
        &[],
    )?;

    let props = instance.properties()?;
    log::info!(
        "loaded OpenXR runtime: {} {}",
        props.runtime_name,
        props.runtime_version
    );

    // Request a form factor from the device (HMD, Handheld, etc.)
    let system = instance.system(openxr::FormFactor::HEAD_MOUNTED_DISPLAY)?;

    // Check what blend mode is valid for this device (opaque vs transparent displays). We'll just
    // take the first one available!
    let blend_mode = instance.enumerate_environment_blend_modes(system, VIEW_TYPE)?[0];

    log::info!(
        "Created OpenXR context with : {:?} {:?}",
        system,
        blend_mode
    );

    Ok(OpenXRContext {
        entry,
        instance,
        props,
        system,
        blend_mode,
    })
}

impl WgpuLoader for OpenXRContext {
    fn load_wgpu(&mut self) -> Option<super::WgpuContext> {
        // OpenXR wants to ensure apps are using the correct graphics card and Vulkan features and
        // extensions, so the instance and device MUST be set up before Instance::create_session.

        let wgpu_limits = wgpu::Limits::downlevel_webgl2_defaults();

        let wgpu_features = wgpu::Features::MULTIVIEW;
        let vk_target_version = TARGET_VULKAN_VERSION; // Vulkan 1.1 guarantees multiview support
        let vk_target_version_xr = xr::Version::new(1, 1, 0);

        let reqs = self
            .instance
            .graphics_requirements::<xr::Vulkan>(self.system)
            .unwrap();

        if vk_target_version_xr < reqs.min_api_version_supported
            || vk_target_version_xr.major() > reqs.max_api_version_supported.major()
        {
            panic!(
                "OpenXR runtime requires Vulkan version > {}, < {}.0.0",
                reqs.min_api_version_supported,
                reqs.max_api_version_supported.major() + 1
            );
        }

        let vk_entry = unsafe { ash::Entry::load().unwrap() };
        log::info!("Successfully loaded Vulkan entry");

        let vk_app_info = vk::ApplicationInfo::builder()
            .application_version(0)
            .engine_version(0)
            .api_version(vk_target_version);

        let mut flags = hal::InstanceFlags::empty();
        #[cfg(debug_assertions)]
        {
            flags |= hal::InstanceFlags::VALIDATION;
            flags |= hal::InstanceFlags::DEBUG;
        }

        let instance_extensions =
            <hal::api::Vulkan as hal::Api>::Instance::required_extensions(&vk_entry, flags)
                .unwrap();

        log::info!("Requested instance extensions: {:?}", instance_extensions);
        let instance_extensions_ptrs: Vec<_> =
            instance_extensions.iter().map(|x| x.as_ptr()).collect();

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&vk_app_info)
            .enabled_extension_names(&instance_extensions_ptrs);

        let vk_instance = unsafe {
            let vk_instance = self
                .instance
                .create_vulkan_instance(
                    self.system,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    &create_info as *const _ as *const _,
                )
                .expect("XR error creating Vulkan instance")
                .map_err(vk::Result::from_raw)
                .expect("Vulkan error creating Vulkan instance");
            ash::Instance::load(
                vk_entry.static_fn(),
                vk::Instance::from_raw(vk_instance as _),
            )
        };

        log::info!("Successfully created Vulkan instance");

        let vk_physical_device = vk::PhysicalDevice::from_raw(unsafe {
            self.instance
                .vulkan_graphics_device(self.system, vk_instance.handle().as_raw() as _)
                .unwrap() as _
        });

        let vk_device_properties =
            unsafe { vk_instance.get_physical_device_properties(vk_physical_device) };
        if vk_device_properties.api_version < vk_target_version {
            unsafe { vk_instance.destroy_instance(None) };
            panic!("Vulkan phyiscal device doesn't support version 1.1");
        }

        log::info!(
            "Got Vulkan physical device with properties {:?}",
            vk_device_properties
        );

        let queue_family_index = unsafe {
            vk_instance
                .get_physical_device_queue_family_properties(vk_physical_device)
                .into_iter()
                .enumerate()
                .find_map(|(queue_family_index, info)| {
                    if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                        Some(queue_family_index as u32)
                    } else {
                        None
                    }
                })
                .expect("Vulkan device has no graphics queue")
        };

        log::info!("Got Vulkan queue family index {}", queue_family_index);

        let hal_instance = unsafe {
            <hal::api::Vulkan as hal::Api>::Instance::from_raw(
                vk_entry.clone(),
                vk_instance.clone(),
                vk_target_version,
                0,
                instance_extensions,
                flags,
                false,
                Some(Box::new(())),
            )
            .unwrap()
        };
        let hal_exposed_adapter = hal_instance.expose_adapter(vk_physical_device).unwrap();

        log::info!("Created WGPU-HAL instance and adapter");

        //TODO actually check if the extensions are available and avoid using then in the loaders
        let mut device_extensions = hal_exposed_adapter
            .adapter
            .required_device_extensions(wgpu_features);

        #[cfg(target_os = "windows")]
        device_extensions.push(ash::extensions::khr::ExternalMemoryWin32::name());

        log::info!("Requested device extensions: {:?}", device_extensions);

        let uab_types = wgpu_hal::UpdateAfterBindTypes::from_limits(
            &wgpu_limits,
            &hal_exposed_adapter
                .adapter
                .physical_device_capabilities()
                .properties()
                .limits,
        );

        let family_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .queue_priorities(&[1.0])
            .build();

        let vk_device =
            {
                unsafe {
                    let device_extensions_ptrs = device_extensions
                        .iter()
                        .map(|x| x.as_ptr())
                        .collect::<Vec<_>>();
                    let mut enabled_features = hal_exposed_adapter
                        .adapter
                        .physical_device_features(&device_extensions, wgpu_features, uab_types);

                    let device_create_info = enabled_features
                        .add_to_device_create_builder(
                            vk::DeviceCreateInfo::builder()
                                .enabled_extension_names(&device_extensions_ptrs)
                                .queue_create_infos(&[family_info])
                                .push_next(&mut vk::PhysicalDeviceMultiviewFeatures {
                                    multiview: vk::TRUE,
                                    ..Default::default()
                                }),
                        )
                        .build();

                    let vk_device = self
                        .instance
                        .create_vulkan_device(
                            self.system,
                            std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                            vk_physical_device.as_raw() as _,
                            &device_create_info as *const _ as *const _,
                        )
                        .expect("XR error creating Vulkan device")
                        .map_err(vk::Result::from_raw)
                        .expect("Vulkan error creating Vulkan device");

                    log::info!("Creating ash vulkan device device from native");
                    ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(vk_device as _))
                }
            };

        let vk_device_ptr = vk_device.handle().as_raw();
        log::info!("Successfully created Vulkan device");

        let hal_device = unsafe {
            hal_exposed_adapter
                .adapter
                .device_from_raw(
                    vk_device,
                    true, //    TODO: is this right?
                    &device_extensions,
                    wgpu_features,
                    uab_types,
                    family_info.queue_family_index,
                    0,
                )
                .unwrap()
        };

        log::info!("Successfully created WGPU-HAL device from vulkan device");

        let wgpu_instance =
            unsafe { wgpu::Instance::from_hal::<wgpu_hal::api::Vulkan>(hal_instance) };
        let wgpu_adapter = unsafe { wgpu_instance.create_adapter_from_hal(hal_exposed_adapter) };
        let (wgpu_device, wgpu_queue) = unsafe {
            wgpu_adapter
                .create_device_from_hal(
                    hal_device,
                    &wgpu::DeviceDescriptor {
                        features: wgpu_features,
                        limits: wgpu_limits,
                        ..Default::default()
                    },
                    None,
                )
                .unwrap()
        };

        log::info!("Successfully created WGPU context");

        Some(super::WgpuContext {
            instance: wgpu_instance,
            device: wgpu_device,
            physical_device: wgpu_adapter,
            queue: wgpu_queue,
            queue_index: family_info.queue_family_index,
            vk_entry,
            vk_device_ptr,
            vk_instance_ptr: vk_instance.handle().as_raw(),
            vk_phys_device_ptr: vk_physical_device.as_raw(),
        })
    }
}

impl WgpuRunner for OpenXRContext {
    fn run(&mut self, _wgpu_context: &super::WgpuContext) {
        todo!()
    }
}

impl OpenXRContext {
    pub fn create_swapchain(
        &self,
        xr_session: &openxr::Session<openxr::Vulkan>,
        device: &Device,
    ) -> (
        openxr::Swapchain<openxr::Vulkan>,
        vk::Extent2D,
        Vec<Texture>,
    ) {
        log::info!("Creating OpenXR swapchain");

        // Fetch the views we need to render to (the eye screens on the HMD)
        let views = self
            .instance
            .enumerate_view_configuration_views(self.system, VIEW_TYPE)
            .unwrap();
        assert_eq!(views.len(), VIEW_COUNT as usize);
        assert_eq!(views[0], views[1]);

        // Create the OpenXR swapchain
        let vk_color_format = vk::Format::B8G8R8A8_SRGB;
        let color_format = wgpu::TextureFormat::Bgra8Unorm;
        let resolution = vk::Extent2D {
            width: views[0].recommended_image_rect_width,
            height: views[0].recommended_image_rect_height,
        };
        let xr_swapchain = xr_session
            .create_swapchain(&openxr::SwapchainCreateInfo {
                create_flags: openxr::SwapchainCreateFlags::EMPTY,
                usage_flags: openxr::SwapchainUsageFlags::COLOR_ATTACHMENT
                    | openxr::SwapchainUsageFlags::SAMPLED,
                format: vk_color_format.as_raw() as _,
                sample_count: 1,
                width: resolution.width,
                height: resolution.height,
                face_count: 1,
                array_size: VIEW_COUNT,
                mip_count: 1,
            })
            .unwrap();

        // Create image views for the swapchain
        let swapcain_textures: Vec<_> = xr_swapchain
            .enumerate_images()
            .unwrap()
            .into_iter()
            .map(|image| {
                let wgpu_tex_desc = wgpu::TextureDescriptor {
                    label: Some("Swapchain Target"),
                    size: Extent3d {
                        width: resolution.width,
                        height: resolution.height,
                        depth_or_array_layers: VIEW_COUNT,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: color_format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                };

                let wgpu_hal_tex_desc = wgpu_hal::TextureDescriptor {
                    label: wgpu_tex_desc.label,
                    size: wgpu_tex_desc.size,
                    mip_level_count: wgpu_tex_desc.mip_level_count,
                    sample_count: wgpu_tex_desc.sample_count,
                    dimension: wgpu_tex_desc.dimension,
                    format: wgpu_tex_desc.format,
                    usage: wgpu_hal::TextureUses::COLOR_TARGET | wgpu_hal::TextureUses::RESOURCE, //todo what here?
                    memory_flags: MemoryFlags::empty(),
                };

                // Create a WGPU image view for this image
                vulkan_image_to_texture(
                    device,
                    vk::Image::from_raw(image),
                    wgpu_tex_desc,
                    wgpu_hal_tex_desc,
                )
            })
            .collect();

        (xr_swapchain, resolution, swapcain_textures)
    }
}
