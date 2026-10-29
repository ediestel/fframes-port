//! A window that Skia draws into, on the GPU (Metal or Vulkan) or the CPU: the part of the
//! player that knows nothing about videos, for other tools that show pictures in a window.

use std::rc::Rc;

use fframes_skia_renderer::render::RenderCache;
use fframes_skia_renderer::skia_safe::Canvas;
use winit::window::Window;

use crate::error::PlayerResult;
use crate::options::PlayerBackend;
use crate::presenter::{WindowTarget, create_target};

// Fields drop in declaration order: the cached Skia objects before the target that owns
// the context and the device.
pub struct Surface {
    render_cache: RenderCache,
    target: Box<dyn WindowTarget>,
    window: Rc<Window>,
}

impl Surface {
    /// Draws into `window` with `backend` (`Auto` tries Metal, then Vulkan, then the CPU).
    ///
    /// # Errors
    /// When the chosen backend can not draw into the window.
    pub fn new(window: Rc<Window>, backend: PlayerBackend, vsync: bool) -> PlayerResult<Self> {
        let target = create_target(&window, backend, vsync)?;
        let mut surface = Self {
            render_cache: target.render_cache(),
            target,
            window,
        };
        surface.resize()?;
        Ok(surface)
    }

    #[must_use]
    pub fn is_gpu(&self) -> bool {
        self.target.is_gpu()
    }

    #[must_use]
    pub fn window(&self) -> &Rc<Window> {
        &self.window
    }

    /// Resizes the window images to the current window size.
    ///
    /// # Errors
    /// When the backend can not create images of the new size.
    pub fn resize(&mut self) -> PlayerResult<()> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            // Minimized, keep the old images around until the window is visible again.
            return Ok(());
        }
        self.target.resize(size.width, size.height)
    }

    /// Draws the next image of the window with `draw` and shows it. `draw` gets the canvas,
    /// in physical pixels, and the cache for `fframes_skia_renderer::render::render_tree`.
    ///
    /// # Errors
    /// When the backend fails to draw or present.
    pub fn present(&mut self, draw: &mut dyn FnMut(&Canvas, &mut RenderCache)) -> PlayerResult<()> {
        let Self {
            render_cache,
            target,
            ..
        } = self;
        target.present(&mut |canvas| draw(canvas, render_cache))
    }
}
