use anyhow::*;
use ash::vk;
use image::GenericImageView;
use wgpu::{Extent3d, TextureDescriptor};
use wgpu_hal::MemoryFlags;

use crate::conversions::{build_view_formats, vulkan_image_to_texture};

use super::{formats::InternalColorFormat, WgpuContext};

pub struct Bound;
pub struct Unbound;

pub struct Texture2D<State> {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: Option<wgpu::BindGroup>,
    pub state: std::marker::PhantomData<State>,
}

impl<State> Texture2D<State> {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
        view_format: Option<InternalColorFormat>,
    ) -> anyhow::Result<Texture2D<Unbound>> {
        let img = image::load_from_memory(bytes)?;
        Ok(Self::from_image(
            device,
            queue,
            &img,
            Some(label),
            view_format,
        )?)
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
        view_format: Option<InternalColorFormat>,
    ) -> anyhow::Result<Texture2D<Unbound>> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();
        let view_formats = build_view_formats(view_format)?;
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &view_formats,
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let (view, sampler) =
            Self::get_view_and_sampler(device, &texture, wgpu::FilterMode::Linear, view_format);

        Ok(Texture2D::<Unbound> {
            texture,
            view,
            sampler,
            bind_group: None,
            state: std::marker::PhantomData,
        })
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn as_render_target_with_extent(
        &self,
        label: &str,
        extent: wgpu::Extent3d,
        format: InternalColorFormat,
        view_format: Option<InternalColorFormat>,
        device: &wgpu::Device,
    ) -> anyhow::Result<Texture2D<Unbound>> {
        let desc = &TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: self.texture.dimension(),
            format: format.try_into()?,
            usage: self.texture.usage() | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &build_view_formats(view_format)?,
        };
        let texture = device.create_texture(desc);
        let (view, sampler) =
            Self::get_view_and_sampler(device, &texture, wgpu::FilterMode::Linear, view_format);
        Ok(Texture2D::<Unbound> {
            texture,
            view,
            sampler,
            bind_group: None,
            state: std::marker::PhantomData,
        })
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn from_wgpu(
        device: &wgpu::Device,
        texture: wgpu::Texture,
        view_format: Option<InternalColorFormat>,
    ) -> Texture2D<Unbound> {
        let (view, sampler) =
            Self::get_view_and_sampler(device, &texture, wgpu::FilterMode::Linear, view_format);
        Texture2D::<Unbound> {
            texture,
            view,
            sampler,
            bind_group: None,
            state: std::marker::PhantomData,
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn from_vk_image(
        label: &str,
        device: &wgpu::Device,
        image: vk::Image,
        size: Extent3d,
        format: InternalColorFormat,
        view_format: Option<InternalColorFormat>,
        usage: wgpu::TextureUsages,
    ) -> anyhow::Result<Texture2D<Unbound>> {
        let view_formats = build_view_formats(view_format)?;

        let wgpu_tex_desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.try_into()?,
            view_formats: &view_formats,
            usage,
        };

        let hal_usage = map_texture_usage(usage, wgpu_tex_desc.format.into())
            | if wgpu_tex_desc.format.is_depth_stencil_format() {
                wgpu_hal::TextureUses::DEPTH_STENCIL_WRITE
            } else if wgpu_tex_desc.usage.contains(wgpu::TextureUsages::COPY_DST) {
                wgpu_hal::TextureUses::COPY_DST // (set already)
            } else {
                wgpu_hal::TextureUses::COLOR_TARGET
            };

        let wgpu_hal_tex_desc = wgpu_hal::TextureDescriptor {
            label: wgpu_tex_desc.label,
            size: wgpu_tex_desc.size,
            mip_level_count: wgpu_tex_desc.mip_level_count,
            sample_count: wgpu_tex_desc.sample_count,
            dimension: wgpu_tex_desc.dimension,
            format: wgpu_tex_desc.format,
            view_formats: view_formats.clone(),
            usage: hal_usage,
            memory_flags: MemoryFlags::empty(),
        };

        // Create a WGPU image view for this image
        let wgpu_texture = vulkan_image_to_texture(device, image, wgpu_tex_desc, wgpu_hal_tex_desc);

        Ok(Self::from_wgpu(device, wgpu_texture, view_format))
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn get_view_and_sampler(
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        filter_mode: wgpu::FilterMode,
        view_format: Option<InternalColorFormat>,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        (
            texture.create_view(&wgpu::TextureViewDescriptor {
                format: view_format.and_then(|f| f.try_into().ok()),
                ..Default::default()
            }),
            device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter_mode,
                min_filter: filter_mode,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }),
        )
    }
}

impl Texture2D<Unbound> {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn bind_to_context(
        self,
        wgpu_context: &WgpuContext,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Texture2D<Bound> {
        Texture2D::<Bound> {
            bind_group: Some(
                wgpu_context
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&self.view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                        ],
                        label: Some("Texture Bind Group"),
                    }),
            ),
            texture: self.texture,
            view: self.view,
            sampler: self.sampler,
            state: std::marker::PhantomData,
        }
    }
}

impl Texture2D<Bound> {
    #[inline]
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.bind_group.as_ref().unwrap()
    }
}

pub struct RoundRobinTextureBuffer<TextureType: Sized, const SIZE: usize> {
    textures: [TextureType; SIZE],
    index: usize,
}

impl<TextureType: Sized, const SIZE: usize> RoundRobinTextureBuffer<TextureType, SIZE> {
    pub fn new(textures: [TextureType; SIZE]) -> Self {
        Self { textures, index: 0 }
    }

    pub fn current(&self) -> &TextureType {
        &self.textures[self.index]
    }

    pub fn previous(&self, idx: usize) -> &TextureType {
        let index = (self.index + SIZE - idx) % SIZE;
        &self.textures[index]
    }

    pub fn next(&mut self) -> &TextureType {
        let index = self.index;
        self.index = (self.index + 1) % SIZE;
        &mut self.textures[index]
    }
}

pub fn map_texture_usage(
    usage: wgpu::TextureUsages,
    aspect: wgpu_hal::FormatAspects,
) -> wgpu_hal::TextureUses {
    let mut u = wgpu_hal::TextureUses::empty();
    u.set(
        wgpu_hal::TextureUses::COPY_SRC,
        usage.contains(wgpu::TextureUsages::COPY_SRC),
    );
    u.set(
        wgpu_hal::TextureUses::COPY_DST,
        usage.contains(wgpu::TextureUsages::COPY_DST),
    );
    u.set(
        wgpu_hal::TextureUses::RESOURCE,
        usage.contains(wgpu::TextureUsages::TEXTURE_BINDING),
    );
    u.set(
        wgpu_hal::TextureUses::STORAGE_READ | wgpu_hal::TextureUses::STORAGE_READ_WRITE,
        usage.contains(wgpu::TextureUsages::STORAGE_BINDING),
    );
    let is_color = aspect.contains(wgpu_hal::FormatAspects::COLOR);
    u.set(
        wgpu_hal::TextureUses::COLOR_TARGET,
        usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT) && is_color,
    );
    u.set(
        wgpu_hal::TextureUses::DEPTH_STENCIL_READ | wgpu_hal::TextureUses::DEPTH_STENCIL_WRITE,
        usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT) && !is_color,
    );
    u
}
