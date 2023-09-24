use std::ffi::c_char;

use anyhow::{bail, Context};
use ash::vk::{self, Handle, QueueGlobalPriorityKHR};

use openxr as xr;
use wgpu::Device;
use wgpu_hal as hal;
use xr::SystemId;

use crate::engine::swapchain::SwapchainCreationInfo;

use super::{
    formats::InternalColorFormat, swapchain::Swapchain, WgpuContext, WgpuLoader, WgpuRunner,
    TARGET_VULKAN_VERSION,
};

pub struct OpenXRContext {
    pub entry: openxr::Entry,
    pub instance: openxr::Instance,
    pub props: openxr::InstanceProperties,
    pub system: openxr::SystemId,
    pub blend_mode: openxr::EnvironmentBlendMode,
}

pub const VIEW_TYPE: openxr::ViewConfigurationType = openxr::ViewConfigurationType::PRIMARY_STEREO;
pub const VIEW_COUNT: u32 = 2;
pub const SWAPCHAIN_COLOR_FORMAT: InternalColorFormat = InternalColorFormat::Bgra8UnormSrgb;

#[cfg(not(feature = "dist"))]
pub fn openxr_layers() -> [&'static str; 0] {
    [] //TODO: ["VK_LAYER_KHRONOS_validation"]
}

#[cfg(feature = "dist")]
pub fn openxr_layers() -> [&'static str; 0] {
    []
}

#[cfg_attr(feature = "profiling", profiling::function)]
pub fn enable_xr_runtime() -> anyhow::Result<OpenXRContext> {
    #[cfg(not(target_os = "android"))]
    let entry = openxr::Entry::linked();
    #[cfg(target_os = "android")]
    let entry = unsafe { openxr::Entry::load()? };

    #[cfg(target_os = "android")]
    entry.initialize_android_loader()?;

    let available_extensions = entry.enumerate_extensions()?;
    log::info!("Available extensions: {:?}", available_extensions);
    assert!(available_extensions.khr_vulkan_enable2);

    let mut enabled_extensions = openxr::ExtensionSet::default();
    enabled_extensions.khr_vulkan_enable2 = true;

    #[cfg(target_os = "android")]
    {
        assert!(available_extensions.khr_android_create_instance);
        enabled_extensions.khr_android_create_instance = true;
    }
    log::info!("Enabled extensions: {:?}", enabled_extensions);

    #[cfg(not(feature = "dist"))]
    {
        for layer in entry.enumerate_layers()? {
            log::info!(
                "[OPENXR] Found layer: {} {:?}",
                layer.layer_name,
                layer.spec_version
            );
        }
    }

    log::info!("Loading OpenXR Runtime...");
    let instance = entry.create_instance(
        &openxr::ApplicationInfo {
            application_name: "VR Screen Viewer",
            application_version: 0,
            engine_name: "void*",
            engine_version: 0,
        },
        &enabled_extensions,
        &openxr_layers(),
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

#[cfg(feature = "dist")]
fn instance_flags() -> hal::InstanceFlags {
    hal::InstanceFlags::empty()
}

#[cfg(not(feature = "dist"))]
fn instance_flags() -> hal::InstanceFlags {
    hal::InstanceFlags::VALIDATION | hal::InstanceFlags::DEBUG
}

#[cfg(not(feature = "dist"))]
fn vulkan_layers() -> Vec<*const c_char> {
    use std::ffi::CStr;

    let layer_names = [CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0").unwrap()];
    layer_names
        .iter()
        .map(|raw_name| raw_name.as_ptr())
        .collect()
}

#[cfg(not(feature = "dist"))]
fn populate_debug_messenger_create_info() -> Option<vk::DebugUtilsMessengerCreateInfoEXT> {
    use std::ptr;

    use crate::utils::validation;

    Some(vk::DebugUtilsMessengerCreateInfoEXT {
        s_type: vk::StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
        p_next: ptr::null(),
        flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
        message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING |
            // vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE |
            // vk::DebugUtilsMessageSeverityFlagsEXT::INFO |
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
        message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
        pfn_user_callback: Some(validation::debug_callback),
        p_user_data: ptr::null_mut(),
    })
}

#[cfg(not(feature = "dist"))]
fn setup_debug_utils(
    entry: &ash::Entry,
    instance: &ash::Instance,
) -> (
    Option<ash::extensions::ext::DebugUtils>,
    Option<vk::DebugUtilsMessengerEXT>,
) {
    let debug_utils_loader = ash::extensions::ext::DebugUtils::new(entry, instance);
    let messenger_ci = populate_debug_messenger_create_info().unwrap();

    let utils_messenger = unsafe {
        debug_utils_loader
            .create_debug_utils_messenger(&messenger_ci, None)
            .expect("Debug Utils Callback")
    };

    (Some(debug_utils_loader), Some(utils_messenger))
}

#[cfg(feature = "dist")]
fn vulkan_layers() -> Vec<*const c_char> {
    vec![]
}

#[cfg(feature = "dist")]
fn populate_debug_messenger_create_info() -> Option<vk::DebugUtilsMessengerCreateInfoEXT> {
    None
}

#[cfg(feature = "dist")]
fn setup_debug_utils(
    _entry: &ash::Entry,
    _instance: &ash::Instance,
) -> (
    Option<ash::extensions::ext::DebugUtils>,
    Option<vk::DebugUtilsMessengerEXT>,
) {
    (None, None)
}

impl WgpuLoader for OpenXRContext {
    fn load_wgpu(&mut self) -> anyhow::Result<super::WgpuContext> {
        // OpenXR wants to ensure apps are using the correct graphics card and Vulkan features and
        // extensions, so the instance and device MUST be set up before Instance::create_session.

        let wgpu_limits = wgpu::Limits::default();

        let wgpu_features = wgpu::Features::MULTIVIEW;
        let vk_target_version = TARGET_VULKAN_VERSION; // Vulkan 1.1 guarantees multiview support
        let vk_target_version_xr = xr::Version::new(1, 1, 0);

        let reqs = self
            .instance
            .graphics_requirements::<xr::Vulkan>(self.system)?;

        if vk_target_version_xr < reqs.min_api_version_supported
            || vk_target_version_xr.major() > reqs.max_api_version_supported.major()
        {
            bail!(
                "OpenXR runtime requires Vulkan version > {}, < {}.0.0",
                reqs.min_api_version_supported,
                reqs.max_api_version_supported.major() + 1
            );
        }

        let vk_entry = unsafe { ash::Entry::load()? };
        log::info!("Successfully loaded Vulkan entry");

        let vk_app_info = vk::ApplicationInfo::builder()
            .application_version(0)
            .engine_version(0)
            .api_version(vk_target_version);

        let flags = instance_flags();

        let instance_extensions = <hal::api::Vulkan as hal::Api>::Instance::desired_extensions(
            &vk_entry,
            vk_target_version,
            flags,
        )?;

        log::info!("Requested instance extensions: {:?}", instance_extensions);
        let instance_extensions_ptrs: Vec<_> =
            instance_extensions.iter().map(|x| x.as_ptr()).collect();

        #[cfg(not(feature = "dist"))]
        {
            for layer in vk_entry.enumerate_instance_layer_properties()? {
                log::info!(
                    "[VULKAN] Found layer: {} {:?}",
                    String::from_utf8_lossy(unsafe {
                        &*(layer.layer_name.as_ref() as *const _ as *const [u8])
                    }),
                    layer.spec_version
                );
            }
        }

        let instance_layers = vulkan_layers();

        // This create info used to debug issues in vk::createInstance and vk::destroyInstance.
        let mut debug_info = populate_debug_messenger_create_info();

        let mut create_info = vk::InstanceCreateInfo::builder()
            .application_info(&vk_app_info)
            .enabled_extension_names(&instance_extensions_ptrs)
            .enabled_layer_names(&instance_layers);

        if let Some(debug_info) = &mut debug_info {
            create_info = create_info.push_next(debug_info);
        }

        let vk_instance = unsafe {
            let vk_instance = self
                .instance
                .create_vulkan_instance(
                    self.system,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    &create_info as *const _ as *const _,
                )
                .context("XR error creating Vulkan instance")?
                .map_err(vk::Result::from_raw)
                .context("Vulkan error creating Vulkan instance")?;
            ash::Instance::load(
                vk_entry.static_fn(),
                vk::Instance::from_raw(vk_instance as _),
            )
        };

        log::info!("Successfully created Vulkan instance");

        let (debug_utils, debug_messenger) = setup_debug_utils(&vk_entry, &vk_instance);

        let vk_physical_device = vk::PhysicalDevice::from_raw(unsafe {
            self.instance
                .vulkan_graphics_device(self.system, vk_instance.handle().as_raw() as _)?
                as _
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
                .context("Vulkan device has no graphics queue")?
        };

        log::info!("Got Vulkan queue family index {}", queue_family_index);

        let hal_instance = unsafe {
            <hal::api::Vulkan as hal::Api>::Instance::from_raw(
                vk_entry.clone(),
                vk_instance.clone(),
                vk_target_version,
                0,
                None,
                instance_extensions,
                flags,
                false,
                Some(Box::new(())),
            )?
        };
        let hal_exposed_adapter = hal_instance
            .expose_adapter(vk_physical_device)
            .context("Cannot expose WGpu-Hal adapter")?;

        log::info!("Created WGPU-HAL instance and adapter");

        let device_create_info = VkDeviceCreateInfo {
            xr_instance: &self.instance.clone(),
            system_id: self.system,
            hal_exposed_adapter: &hal_exposed_adapter,
            wgpu_features,
            queue_family_index,
            vk_entry: &vk_entry,
            vk_physical_device,
            vk_instance: &vk_instance,
        };

        let mut global_queue_priority = vk::DeviceQueueGlobalPriorityCreateInfoKHR::builder()
            .global_priority(QueueGlobalPriorityKHR::HIGH);

        let mut device_creation_result =
            create_vk_device(device_create_info, Some(&mut global_queue_priority));

        if device_creation_result.is_err() {
            log::warn!("Failed to create Vulkan device with realtime priority, trying without");
            device_creation_result = create_vk_device(device_create_info, None);
        }

        let (device_extensions, family_info, vk_device) = device_creation_result?;

        let vk_device_ptr = vk_device.handle().as_raw();
        log::info!("Successfully created Vulkan device");

        let hal_device = unsafe {
            hal_exposed_adapter.adapter.device_from_raw(
                vk_device,
                true, //    TODO: is this right?
                &device_extensions,
                wgpu_features,
                family_info.queue_family_index,
                0,
            )?
        };

        log::info!("Successfully created WGPU-HAL device from vulkan device");

        let wgpu_instance =
            unsafe { wgpu::Instance::from_hal::<wgpu_hal::api::Vulkan>(hal_instance) };
        let wgpu_adapter = unsafe { wgpu_instance.create_adapter_from_hal(hal_exposed_adapter) };
        let (wgpu_device, wgpu_queue) = unsafe {
            wgpu_adapter.create_device_from_hal(
                hal_device,
                &wgpu::DeviceDescriptor {
                    features: wgpu_features,
                    limits: wgpu_limits,
                    label: None,
                },
                None,
            )?
        };

        log::info!("Successfully created WGPU context");

        log::info!(
            "Queue timestamp period: {}",
            wgpu_queue.get_timestamp_period()
        );

        Ok(super::WgpuContext {
            instance: wgpu_instance,
            device: wgpu_device,
            physical_device: wgpu_adapter,
            queue: wgpu_queue,
            family_queue_index: family_info.queue_family_index,
            vk_entry,
            vk_device_ptr,
            vk_instance_ptr: vk_instance.handle().as_raw(),
            vk_phys_device_ptr: vk_physical_device.as_raw(),
            debug_messenger,
            debug_utils,
        })
    }
}

impl Drop for WgpuContext {
    fn drop(&mut self) {
        if let (Some(debug_messenger), Some(debug_utils_loader)) =
            (self.debug_messenger, self.debug_utils.take())
        {
            unsafe {
                debug_utils_loader.destroy_debug_utils_messenger(debug_messenger, None);
            }
        }
    }
}

impl WgpuRunner for OpenXRContext {
    fn run(&mut self, _wgpu_context: &super::WgpuContext) {
        todo!()
    }
}

impl OpenXRContext {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn create_swapchain(
        &self,
        xr_session: &openxr::Session<openxr::Vulkan>,
        device: &Device,
    ) -> anyhow::Result<(Swapchain, vk::Extent2D)> {
        log::info!("Creating OpenXR swapchain");

        // Fetch the views we need to render to (the eye screens on the HMD)
        let views = self
            .instance
            .enumerate_view_configuration_views(self.system, VIEW_TYPE)?;
        assert_eq!(views.len(), VIEW_COUNT as usize);
        assert_eq!(views[0], views[1]);

        // Create the OpenXR swapchain
        let resolution = vk::Extent2D {
            width: views[0].recommended_image_rect_width,
            height: views[0].recommended_image_rect_height,
        };

        //TODO: Enumerate swapchain formats and pick the best one, remember that WGPU gamma corrects everything
        let color_swapchain = Swapchain::new(
            "OpenXR Swapchain Image",
            xr_session,
            device,
            SwapchainCreationInfo {
                resolution,
                format: SWAPCHAIN_COLOR_FORMAT,
                view_format: Some(SWAPCHAIN_COLOR_FORMAT.to_norm()),
                usage_flags: openxr::SwapchainUsageFlags::COLOR_ATTACHMENT,
                view_count: VIEW_COUNT,
            },
        )?;

        if color_swapchain.is_empty() {
            return Err(anyhow::anyhow!("No swapchain images"));
        }

        Ok((color_swapchain, resolution))
    }
}

#[derive(Clone, Copy)]
struct VkDeviceCreateInfo<'a> {
    xr_instance: &'a openxr::Instance,
    system_id: SystemId,
    hal_exposed_adapter: &'a hal::ExposedAdapter<hal::vulkan::Api>,
    wgpu_features: wgpu::Features,
    queue_family_index: u32,
    vk_entry: &'a ash::Entry,
    vk_physical_device: vk::PhysicalDevice,
    vk_instance: &'a ash::Instance,
}

fn create_vk_device<'a>(
    create_info: VkDeviceCreateInfo<'_>,
    global_queue_priority: Option<&'a mut vk::DeviceQueueGlobalPriorityCreateInfoKHR>,
) -> Result<
    (
        Vec<&'static std::ffi::CStr>,
        vk::DeviceQueueCreateInfoBuilder<'a>,
        ash::Device,
    ),
    anyhow::Error,
> {
    let VkDeviceCreateInfo {
        xr_instance,
        system_id,
        hal_exposed_adapter,
        wgpu_features,
        queue_family_index,
        vk_entry,
        vk_physical_device,
        vk_instance,
    } = create_info;

    let mut device_extensions = hal_exposed_adapter
        .adapter
        .required_device_extensions(wgpu_features);
    #[cfg(target_os = "windows")]
    device_extensions.push(ash::extensions::khr::ExternalMemoryWin32::name());
    log::info!("Requested device extensions: {:?}", device_extensions);
    let mut family_info = vk::DeviceQueueCreateInfo::builder()
        .queue_family_index(queue_family_index)
        .queue_priorities(&[1.0]);

    if let Some(global_queue_priority) = global_queue_priority {
        family_info = family_info.push_next(global_queue_priority);
    }

    let device_extensions_ptrs = device_extensions
        .iter()
        .map(|x| x.as_ptr())
        .collect::<Vec<_>>();
    let mut enabled_features = hal_exposed_adapter
        .adapter
        .physical_device_features(&device_extensions, wgpu_features);
    let mut multiview_params = vk::PhysicalDeviceMultiviewFeatures {
        multiview: vk::TRUE,
        ..Default::default()
    };
    let device_create_info = enabled_features
        .add_to_device_create_builder(vk::DeviceCreateInfo::builder())
        .queue_create_infos(std::slice::from_ref(&family_info))
        .enabled_extension_names(&device_extensions_ptrs)
        .push_next(&mut multiview_params);
    let vk_device = {
        unsafe {
            let vk_device = xr_instance
                .create_vulkan_device(
                    system_id,
                    std::mem::transmute(vk_entry.static_fn().get_instance_proc_addr),
                    vk_physical_device.as_raw() as _,
                    &device_create_info as *const _ as *const _,
                )
                .context("XR error creating Vulkan device")?
                .map_err(vk::Result::from_raw)
                .context("Vulkan error creating Vulkan device")?;

            log::info!("Creating ash vulkan device device from native");
            ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(vk_device as _))
        }
    };
    Ok((device_extensions, family_info, vk_device))
}
