//! Handing GPU memory back as soon as it is let go, in a browser.
//!
//! Natively, dropping a `wgpu::Texture` or `wgpu::Buffer` frees its
//! memory. In a browser (wgpu's WebGPU backend) a drop only lets the
//! JavaScript object go, and the graphics memory stays until the page's
//! garbage collector gets round to it. Safari's collector doesn't see that
//! memory at all, so things made and dropped over and over (the bike
//! turning in the menus, made anew for every bike, colour and outfit) pile
//! up until the GPU gives out and the picture freezes. [`Held`] keeps what
//! a part made and destroys it when the part goes.

/// Textures and buffers destroyed when this is dropped.
#[derive(Default)]
pub struct Held {
    textures: Vec<wgpu::Texture>,
    buffers: Vec<wgpu::Buffer>,
}

impl Held {
    /// Keeps `texture` to destroy with the rest, and gives it back.
    pub fn texture(&mut self, texture: wgpu::Texture) -> wgpu::Texture {
        self.textures.push(texture.clone());
        texture
    }

    /// Keeps `buffer` to destroy with the rest, and gives it back.
    pub fn buffer(&mut self, buffer: wgpu::Buffer) -> wgpu::Buffer {
        self.buffers.push(buffer.clone());
        buffer
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        // Work already sent to the GPU still finishes: WebGPU frees the
        // memory once it has (and natively a drop frees it anyway).
        for t in self.textures.drain(..) {
            t.destroy();
        }
        for b in self.buffers.drain(..) {
            b.destroy();
        }
    }
}
