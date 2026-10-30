//! Black bars at the sides of a window wider than 21:9: the race is drawn
//! 21:9 the window's height and copied into the middle, so the view and
//! its display stay as they are on a 21:9 screen.

/// The widest the race is drawn (21:9, as 3840×1600).
pub const WIDEST: f32 = 12.0 / 5.0;

/// Where the race goes across a `width × height` window: how far in from
/// the left, and how wide.
pub fn race_area((width, height): (u32, u32)) -> (u32, u32) {
    let widest = (height as f32 * WIDEST).round() as u32;
    match width > widest {
        true => ((width - widest) / 2, widest),
        false => (0, width),
    }
}

/// The 21:9 picture the race is drawn on when the window is wider.
#[derive(Default)]
pub struct Bars {
    picture: Option<wgpu::Texture>,
}

impl Bars {
    /// What to draw the race on in a `size` window, and its size: none when
    /// the window is 21:9 or narrower and the race goes straight on it.
    pub fn target(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Option<(wgpu::TextureView, (u32, u32))> {
        let (x, width) = race_area(size);
        if x == 0 {
            self.picture = None;
            return None;
        }
        let fresh = |t: &wgpu::Texture| t.width() == width && t.height() == size.1 && t.format() == format;
        if !self.picture.as_ref().is_some_and(fresh) {
            self.picture = Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("race between the bars"),
                size: wgpu::Extent3d {
                    width,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        }
        let picture = self.picture.as_ref()?;
        Some((picture.create_view(&Default::default()), (width, size.1)))
    }

    /// Clears `window` black and copies the race into its middle.
    pub fn finish(&self, encoder: &mut wgpu::CommandEncoder, window: &wgpu::Texture) {
        let Some(picture) = &self.picture else { return };
        let view = window.create_view(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("black bars"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        let (x, _) = race_area((window.width(), window.height()));
        encoder.copy_texture_to_texture(
            picture.as_image_copy(),
            wgpu::TexelCopyTextureInfo {
                texture: window,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            picture.size(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_only_past_21_by_9() {
        assert_eq!(race_area((1920, 1080)), (0, 1920));
        assert_eq!(race_area((3440, 1440)), (0, 3440));
        assert_eq!(race_area((3840, 1600)), (0, 3840));
        // 32:9: a 21:9 picture in the middle.
        assert_eq!(race_area((5120, 1440)), (832, 3456));
    }
}
