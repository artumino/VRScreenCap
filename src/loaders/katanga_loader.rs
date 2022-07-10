use std::{ffi::c_void, ptr, error::Error};

use windows::{
    Win32::{
        Foundation::{HANDLE, CloseHandle},
        System::Memory::{
            MapViewOfFile, OpenFileMappingA, FILE_MAP_ALL_ACCESS, UnmapViewOfFile,
        },
    },
};

use super::Loader;

pub struct KatangaLoaderContext {
    katanga_file_handle: HANDLE,
    katanga_file_mapping: *mut c_void,
}

impl Loader for KatangaLoaderContext {
    fn load(&mut self) -> Result<(), Box<dyn Error>> {
        self.katanga_file_handle =
        unsafe { OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, "Local\\KatangaMappedFile")? };

        println!("Handle: {:?}", self.katanga_file_handle);

        self.katanga_file_mapping = unsafe { MapViewOfFile(self.katanga_file_handle, FILE_MAP_ALL_ACCESS, 0, 0, 4) };
        if self.katanga_file_mapping.is_null() {
            return Err("Cannot map file!".into());
        }

        let address = unsafe { *(self.katanga_file_mapping as *mut usize) };
        println!("{:#01x}", address);

        Ok(())
    }
}

impl Default for KatangaLoaderContext {
    fn default() -> Self {
        Self { katanga_file_handle: Default::default(), katanga_file_mapping: ptr::null_mut() }
    }
}

impl Drop for KatangaLoaderContext {
    fn drop(&mut self) {
        println!("Dropping KatangaLoaderContext");

        if !self.katanga_file_mapping.is_null() && unsafe { bool::from(UnmapViewOfFile(self.katanga_file_mapping)) } {
            println!("Unmapped file!");
        }
    
        if !self.katanga_file_handle.is_invalid() && unsafe { bool::from(CloseHandle(self.katanga_file_handle)) } {
            println!("Closed handle!");
        }
    }
}