use image::GenericImageView;
use anyhow::*;

pub struct Texture2D {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl Texture2D {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8], 
        label: &str
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(device, queue, &img, Some(label))
    }

    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &image::DynamicImage,
        label: Option<&str>
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(
            &wgpu::TextureDescriptor {
                label,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }
        );

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

        let (view , sampler) = Self::get_view_and_sampler(device, &texture);
        
        Ok(Self { texture, view, sampler })
    }

    pub fn from_wgpu(device: &wgpu::Device,
                     texture: wgpu::Texture) -> Self {
        let (view , sampler) = Self::get_view_and_sampler(device, &texture);
        Self { 
            texture, 
            view, 
            sampler 
        }
    }

    fn get_view_and_sampler(device: &wgpu::Device, texture: &wgpu::Texture) -> (wgpu::TextureView, wgpu::Sampler) {
        let layers = texture.depth_or_array_layers();
        let dimension = if layers > 1 {
            wgpu::TextureViewDimension::D2Array
        } else {
            wgpu::TextureViewDimension::D2
        };

        (
            texture.create_view(&wgpu::TextureViewDescriptor{
                base_array_layer: 0,
                array_layer_count: Some(layers),
                dimension: Some(dimension),
                format: Some(texture.format()),
                base_mip_level: 0,
                mip_level_count: Some(texture.mip_level_count()),
                ..Default::default()
            }),
            device.create_sampler(
                &wgpu::SamplerDescriptor {
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Nearest,
                    ..Default::default()
                }
            )
        )
    }
}