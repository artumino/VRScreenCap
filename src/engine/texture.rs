use anyhow::*;
use image::GenericImageView;
use wgpu::TextureDescriptor;

use super::WgpuContext;

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
    ) -> anyhow::Result<Texture2D<Unbound>> {
        let img = image::load_from_memory(bytes)?;
        Ok(Self::from_image(device, queue, &img, Some(label)))
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>,
    ) -> Texture2D<Unbound> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

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
            view_formats: &[],
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
            Self::get_view_and_sampler(device, &texture, wgpu::FilterMode::Linear);

        Texture2D::<Unbound> {
            texture,
            view,
            sampler,
            bind_group: None,
            state: std::marker::PhantomData,
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn as_render_target_with_extent(
        &self,
        label: &str,
        extent: wgpu::Extent3d,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
    ) -> Texture2D<Unbound> {
        let desc = &TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: self.texture.dimension(),
            format: format,
            usage: self.texture.usage() | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        let texture = device.create_texture(desc);
        let (view, sampler) =
            Self::get_view_and_sampler(device, &texture, wgpu::FilterMode::Linear);
        Texture2D::<Unbound> {
            texture,
            view,
            sampler,
            bind_group: None,
            state: std::marker::PhantomData,
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn from_wgpu(device: &wgpu::Device, texture: wgpu::Texture) -> Texture2D<Unbound> {
        let (view, sampler) =
            Self::get_view_and_sampler(device, &texture, wgpu::FilterMode::Linear);
        Texture2D::<Unbound> {
            texture,
            view,
            sampler,
            bind_group: None,
            state: std::marker::PhantomData,
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn get_view_and_sampler(
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        filter_mode: wgpu::FilterMode,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        (
            texture.create_view(&wgpu::TextureViewDescriptor::default()),
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
