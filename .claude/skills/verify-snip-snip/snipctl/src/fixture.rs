//! The fixture: a borderless, topmost window at screen (100,100), 800x500
//! screen units, left half blue and right half green, so snips of it have
//! known pixels. `target` runs it as a child `snipctl fixture-window`.

use std::{num::NonZeroU32, rc::Rc};

use anyhow::Result;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId, WindowLevel},
};

use crate::{platform, screen::COLORS};

pub const TITLE: &str = "snip-snip verify fixture";
pub const BOUNDS: (i32, i32, u32, u32) = (100, 100, 800, 500);

pub fn run() -> Result<()> {
    EventLoop::new()?.run_app(&mut Fixture::default())?;
    Ok(())
}

#[derive(Default)]
struct Fixture {
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
}

impl ApplicationHandler for Fixture {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (x, y, w, h) = BOUNDS;
        let (position, size): (Position, Size) = if platform::PHYSICAL_SCREEN_UNITS {
            (PhysicalPosition::new(x, y).into(), PhysicalSize::new(w, h).into())
        } else {
            (LogicalPosition::new(x, y).into(), LogicalSize::new(w, h).into())
        };
        let attributes = Window::default_attributes()
            .with_title(TITLE)
            .with_position(position)
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);
        let window = Rc::new(
            event_loop
                .create_window(attributes)
                .expect("creating fixture window"),
        );
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        self.surface = Some(softbuffer::Surface::new(&context, window).expect("softbuffer surface"));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Some(surface) = &mut self.surface {
                    paint(surface).expect("painting fixture");
                }
            }
            _ => {}
        }
    }
}

/// Fills the left half blue and the right half green, in physical pixels.
fn paint(surface: &mut softbuffer::Surface<Rc<Window>, Rc<Window>>) -> Result<()> {
    let size = surface.window().inner_size();
    let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
        return Ok(());
    };
    surface.resize(w, h).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut buffer = surface.buffer_mut().map_err(|e| anyhow::anyhow!("{e}"))?;
    let pack = |[r, g, b]: [u8; 3]| u32::from_be_bytes([0, r, g, b]);
    let (blue, green) = (pack(COLORS[0].1), pack(COLORS[1].1));
    for (i, pixel) in buffer.iter_mut().enumerate() {
        *pixel = if (i as u32 % size.width) < size.width / 2 {
            blue
        } else {
            green
        };
    }
    buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
