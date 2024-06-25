use alloc::boxed::Box;
use alloc::rc::Rc;

use super::CLOCKS;

use embedded_graphics::{
    pixelcolor::raw::RawU16, pixelcolor::Rgb565, prelude::*, primitives::Rectangle,
};

use esp_hal::delay::Delay;
use esp_hal::timer::systimer::SystemTimer;

use display_interface_spi::SPIInterface;
use mipidsi::{
    models::ST7789,
    options::{ColorInversion, Orientation, Rotation},
    Builder,
};

use embassy_time::{Duration, Timer};
use embedded_graphics_core::draw_target::DrawTarget;

use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;

use esp_println::println;

// Display
const LCD_WIDTH: u16 = 240;
const LCD_HEIGHT: u16 = 280;
const LCD_OFFSET_X: u16 = 0;
const LCD_OFFSET_Y: u16 = 20;

slint::include_modules!();

#[global_allocator]
static ALLOCATOR: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

fn init_heap() {
    const HEAP_SIZE: usize = 250 * 1024;
    static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe { ALLOCATOR.init(core::ptr::addr_of_mut!(HEAP) as *mut u8, HEAP_SIZE) }
}

struct EspBackend {
    window: Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
}

impl slint::platform::Platform for EspBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(
            SystemTimer::now() / (SystemTimer::TICKS_PER_SECOND / 1000),
        )
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        esp_println::println!("{}", arguments);
    }
}

struct DisplayWrapper<'a, T> {
    display: &'a mut T,
    line_buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
}

impl<T: DrawTarget<Color = Rgb565>> slint::platform::software_renderer::LineBufferProvider
    for DisplayWrapper<'_, T>
{
    type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;
    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        // Render into the line
        render_fn(&mut self.line_buffer[range.clone()]);

        self.display
            .fill_contiguous(
                &Rectangle::new(
                    Point::new(range.start as _, line as _),
                    Size::new(range.len() as _, 1),
                ),
                self.line_buffer[range.clone()]
                    .iter()
                    .map(|p| RawU16::new(p.0).into()),
            )
            .map_err(drop)
            .unwrap();
    }
}

#[embassy_executor::task]
pub async fn run(
    spidev: impl SpiDevice + 'static,
    dc: impl OutputPin + 'static,
    rst: impl OutputPin + 'static,
) {
    init_heap();

    let mut delay: Delay;

    {
        let mut clocks_unlocked = CLOCKS.lock().await;
        if let Some(clocks) = clocks_unlocked.as_mut() {
            delay = Delay::new(&clocks);
        } else {
            panic!();
        }
    }

    let di = SPIInterface::new(spidev, dc);
    let mut display = Builder::new(ST7789, di)
        .reset_pin(rst)
        .display_size(LCD_WIDTH, LCD_HEIGHT)
        .invert_colors(ColorInversion::Inverted)
        // .color_order(ColorOrder::Bgr)
        .display_offset(LCD_OFFSET_X, LCD_OFFSET_Y)
        .init(&mut delay)
        .unwrap();

    // let orientation = Orientation::new().rotate(Rotation::Deg90);

    let mut line_buffer = [slint::platform::software_renderer::Rgb565Pixel(0); LCD_WIDTH as usize];

    let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
        slint::platform::software_renderer::RepaintBufferType::ReusedBuffer,
    );

    window.set_size(slint::PhysicalSize::new(
        LCD_WIDTH as u32,
        LCD_HEIGHT as u32,
    ));

    slint::platform::set_platform(Box::new(EspBackend {
        window: window.clone(),
    }))
    .expect("backend already initialized");

    let demo = Demo::new().unwrap();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        core::time::Duration::from_secs(2),
        move || {
            let check = demo.get_check();
            demo.set_check(!check);
        },
    );

    loop {
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|renderer| {
            renderer.render_by_line(DisplayWrapper {
                display: &mut display,
                line_buffer: &mut line_buffer,
            });
        });

        if !window.has_active_animations() {
            if let Some(duration) = slint::platform::duration_until_next_timer_update() {
                Timer::after(Duration::from_millis(duration.as_millis() as u64)).await;
                continue;
            }
        }
        Timer::after(Duration::from_millis(10)).await;
    }
}
