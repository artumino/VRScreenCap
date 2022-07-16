use std::{error::Error, ffi::{c_void, CString}, ptr};

use ash::{vk::{self, ImageCreateInfo, PFN_vkGetMemoryWin32HandlePropertiesKHR}, extensions::khr::ExternalMemoryWin32};
use wgpu::{Device, Instance, Texture, TextureUsages};
use wgpu_hal::{api::Vulkan, TextureDescriptor, TextureUses, MemoryFlags};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::Memory::{MapViewOfFile, OpenFileMappingA, UnmapViewOfFile, FILE_MAP_ALL_ACCESS},
};

use super::Loader;

pub struct KatangaLoaderContext {
    katanga_file_handle: HANDLE,
    katanga_file_mapping: *mut c_void,
}

impl Loader for KatangaLoaderContext {
    fn load(&mut self, instance: &Instance, device: &Device) -> Result<wgpu::Texture, Box<dyn Error>> {
        self.katanga_file_handle =
            unsafe { OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, "Local\\KatangaMappedFile")? };
        println!("Handle: {:?}", self.katanga_file_handle);

        self.katanga_file_mapping =
            unsafe { MapViewOfFile(self.katanga_file_handle, FILE_MAP_ALL_ACCESS, 0, 0, 4) };
        if self.katanga_file_mapping.is_null() {
            return Err("Cannot map file!".into());
        }

        let address = unsafe { *(self.katanga_file_mapping as *mut usize) };
        println!("{:#01x}", address | 0xFFFFFFFF00000000);


        let raw_image = unsafe {
            instance.as_hal::<Vulkan, _, _>(|instance| {
                instance.map(|instance| {
                let raw_instance = instance.shared_instance().raw_instance();
                device.as_hal::<Vulkan, _, _>(|device| {
                    device
                        .map(|device| {
                            let raw_device = device.raw_device();
                            let tex_handle = (address | 0xFFFFFFFF00000000) as vk::HANDLE;
                            //let raw_phys_device = device.raw_physical_device();
                            let handle_type = vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE_KMT;

                            //FIXME: Find out how...
                            //let mem_ext = ash::extensions::khr::ExternalMemoryWin32::new(raw_instance, raw_device);

                            //let mem_ext = vk::KhrExternalMemoryWin32Fn::load(|p_name| {
                            //    raw_instance.get_device_proc_addr(raw_device.handle(), p_name.as_ptr()).unwrap() as *const c_void
                            //});
                            //let mut tex_properties = vk::MemoryWin32HandlePropertiesKHR::default();
                            //(mem_ext.get_memory_win32_handle_properties_khr)(raw_device.handle(), handle_type, tex_handle, &mut tex_properties).result().unwrap();
                            //println!("{:#01x}", tex_properties.memory_type_bits);
                            
                            //let handle_properties = mem_ext.get_memory_win32_handle_properties(vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE_KMT, (address | 0xFFFFFFFF00000000) as vk::HANDLE).unwrap();
                            
                            //let mut dedicated_creation_info = vk::DedicatedAllocationImageCreateInfoNV::builder()
                            //    .dedicated_allocation(true);

                            let mut ext_create_info = vk::ExternalMemoryImageCreateInfo::builder()
                                .handle_types(handle_type);
                                
                            let image_create_info = ImageCreateInfo::builder()
                                .push_next(&mut ext_create_info)
                                //.push_next(&mut dedicated_creation_info)
                                .image_type(vk::ImageType::TYPE_2D)
                                .format(vk::Format::R8G8B8A8_UNORM)
                                .extent(vk::Extent3D {
                                    width: 3840,
                                    height: 1080,
                                    depth: 1,
                                })
                                .mip_levels(1)
                                .array_layers(1)
                                .samples(vk::SampleCountFlags::TYPE_1)
                                .tiling(vk::ImageTiling::OPTIMAL)
                                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                                .sharing_mode(vk::SharingMode::EXCLUSIVE);

                            raw_device
                                .create_image(&image_create_info, None)
                                .map(|raw_image| {

                                    let img_req = raw_device.get_image_memory_requirements(raw_image);

                                    
                                    //let mut dedicated_allocate_info = vk::DedicatedAllocationMemoryAllocateInfoNV::builder()
                                    //    .image(raw_image);

                                    let mut import_memory_info =
                                        vk::ImportMemoryWin32HandleInfoKHR::builder()
                                            .handle_type(
                                                handle_type,
                                            )
                                            .handle((address | 0xFFFFFFFF00000000) as vk::HANDLE);
                                            //.handle(self.katanga_file_mapping);

                                    let allocate_info = vk::MemoryAllocateInfo::builder()
                                        .push_next(&mut import_memory_info)
                                        //.push_next(&mut dedicated_allocate_info)
                                        .allocation_size(img_req.size)
                                        .memory_type_index(0);
                                    raw_device
                                        .allocate_memory(&allocate_info, None)
                                        .map(|allocated_memory| {
                                            raw_device
                                                .bind_image_memory(raw_image, allocated_memory, 0)
                                                .map(|_bound_image| raw_image)
                                                .unwrap()
                                    })
                                })
                        }).unwrap()
                    })
                }).unwrap()
            })
        }??;

        let text_descriptor = TextureDescriptor {
            label: "KatangaStream".into(),
            size: wgpu::Extent3d { width: 3840, height: 1080, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: TextureUses::EXCLUSIVE,
            memory_flags: MemoryFlags::empty(),
        };

        let texture = unsafe { <wgpu_hal::api::Vulkan as wgpu_hal::Api>::Device::texture_from_raw(raw_image, &text_descriptor, None) };
        //Texture::from_raw(raw_image);
        //device.create_texture_from_hal::<Vulkan, _, _>(raw_image, desc)
        Ok(unsafe { device.create_texture_from_hal::<Vulkan>(texture, &wgpu::TextureDescriptor {
            label: "KatangaStream".into(),
            size: wgpu::Extent3d { width: 3840, height: 1080, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
        })})
    }
}

impl Default for KatangaLoaderContext {
    fn default() -> Self {
        Self {
            katanga_file_handle: Default::default(),
            katanga_file_mapping: ptr::null_mut(),
        }
    }
}

impl Drop for KatangaLoaderContext {
    fn drop(&mut self) {
        println!("Dropping KatangaLoaderContext");

        if !self.katanga_file_mapping.is_null()
            && unsafe { bool::from(UnmapViewOfFile(self.katanga_file_mapping)) }
        {
            println!("Unmapped file!");
        }

        if !self.katanga_file_handle.is_invalid()
            && unsafe { bool::from(CloseHandle(self.katanga_file_handle)) }
        {
            println!("Closed handle!");
        }
    }
}
