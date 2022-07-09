use windows::{
    Win32::{
        Foundation::{CloseHandle},
        System::Memory::{
            MapViewOfFile, OpenFileMappingA, FILE_MAP_ALL_ACCESS, UnmapViewOfFile,
        },
    },
};

use super::Loader;

pub struct KatangaLoaderContext {}

impl Loader for KatangaLoaderContext {
    fn load(&self) -> bool {
        let handle =
        unsafe { OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, "Local\\KatangaMappedFile") }
            .expect("Cannot open katanga file!");

        println!("Handle: {:?}", handle);

        let file_view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, 4) };
        if file_view as usize == 0 {
            panic!("Cannot map file!");
        }
        let address = unsafe { *(file_view as *mut usize).as_ref().unwrap() };
        println!("{:#01x}", address);

        if unsafe { bool::from(UnmapViewOfFile(file_view)) } {
            println!("Unmapped file!");
        }

        if unsafe { bool::from(CloseHandle(handle)) } {
            println!("Closed handle!");
        }

        true
    }
}