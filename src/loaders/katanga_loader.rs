use std::{error::Error, ffi::c_void, ptr};

use ash::vk::{self, ImageCreateInfo};
use wgpu::{Device, Instance};
use wgpu_hal::api::Vulkan;
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
    fn load(&mut self, _instance: &Instance, device: &Device) -> Result<(), Box<dyn Error>> {
        self.katanga_file_handle =
            unsafe { OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, "Local\\KatangaMappedFile")? };
        println!("Handle: {:?}", self.katanga_file_handle);

        self.katanga_file_mapping =
            unsafe { MapViewOfFile(self.katanga_file_handle, FILE_MAP_ALL_ACCESS, 0, 0, 4) };
        if self.katanga_file_mapping.is_null() {
            return Err("Cannot map file!".into());
        }

        let address = unsafe { *(self.katanga_file_mapping as *mut isize) };
        println!("{:#01x}", address);

        //let raw_instance = unsafe {
        //    instance.as_hal::<Vulkan, _, _>(|instance| {
        //        instance.map(|instance| {
        //            instance.shared_instance().raw_instance()
        //        }).unwrap()
        //    })
        //};

        let _raw_image = unsafe {
            device.as_hal::<Vulkan, _, _>(|device| {
                device
                    .map(|device| {
                        let raw_device = device.raw_device();
                        //let raw_phys_device = device.raw_physical_device();
                        let mut ext_create_info = vk::ExternalMemoryImageCreateInfo::builder()
                            .handle_types(vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE);
                        let image_create_info = ImageCreateInfo::builder()
                            .push_next(&mut ext_create_info)
                            .image_type(vk::ImageType::TYPE_2D)
                            .format(vk::Format::R8G8B8A8_UNORM)
                            .extent(vk::Extent3D {
                                width: 3840,
                                height: 1080,
                                depth: 28,
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
                                let mut import_memory_info =
                                    vk::ImportMemoryWin32HandleInfoKHR::builder()
                                        .handle_type(
                                            vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE,
                                        )
                                        .handle(self.katanga_file_mapping);

                                let img_req = raw_device.get_image_memory_requirements(raw_image);

                                let allocate_info = vk::MemoryAllocateInfo::builder()
                                    .push_next(&mut import_memory_info)
                                    .allocation_size(img_req.size);
                                //TODO: .memory_type_index(memory_type_index);

                                raw_device
                                    .allocate_memory(&allocate_info, None)
                                    .map(|allocated_memory| {
                                        raw_device
                                            .bind_image_memory(raw_image, allocated_memory, 0)
                                            .map(|_bound_image| raw_image)
                                            .unwrap()
                                    })
                                    .unwrap()
                            })
                    })
                    .unwrap()
            })
        }?;
        Ok(())
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
