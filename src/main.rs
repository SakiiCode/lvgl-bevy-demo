use std::{
    ffi::CString,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565, prelude::Point};
use esp_idf_svc::hal::{
    delay::Delay,
    gpio::PinDriver,
    peripherals::Peripherals,
    spi::{
        config::{Config, DriverConfig},
        Dma, SpiDeviceDriver,
    },
    units::MegaHertz,
};
use esp_idf_svc::sys::{self as esp_idf_sys, xTaskGetTickCount, MALLOC_CAP_DEFAULT};
use lv_bevy_ecs::{
    display::{Display, DrawBuffer},
    error,
    events::EventCode,
    functions::*,
    info,
    input::{BufferStatus, InputDevice, InputEvent, InputState, Pointer},
    malloc::set_mem_monitor,
    support::{Align, LabelLongMode},
    sys::{lv_mem_monitor_t, lv_tick_set_cb, LV_DEF_REFR_PERIOD},
    warn,
    widgets::{Arc, Label, Wdg},
};
use mipidsi::{interface::SpiInterface, models::ST7789, Builder};
use static_cell::StaticCell;
use xpt2046::{TouchEvent, TouchKind, TouchScreen, Xpt2046};

pub fn get_memory_stats(monitor: &mut lv_mem_monitor_t) {
    unsafe {
        static mut MAX_USED: u32 = 0;

        let total = esp_idf_sys::heap_caps_get_total_size(MALLOC_CAP_DEFAULT);
        monitor.total_size = total;

        let free = esp_idf_sys::heap_caps_get_free_size(MALLOC_CAP_DEFAULT);
        monitor.free_size = free;

        let largest_free = esp_idf_sys::heap_caps_get_largest_free_block(MALLOC_CAP_DEFAULT);
        monitor.free_biggest_size = largest_free;

        let used = total - free;
        let new_max = u32::max(MAX_USED, used as u32);
        MAX_USED = new_max;
        monitor.max_used = new_max as usize;

        let used_pct = (used) * 100 / total;
        monitor.used_pct = used_pct as u8;
        let frag_pct = (total - largest_free) * 100 / total;
        monitor.frag_pct = frag_pct as u8;
    }
}

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    // Forward LVGL logs to EspLogger
    lv_bevy_ecs::logging::connect();

    // Use LVGL logger instead
    //lv_log_init();

    set_mem_monitor(get_memory_stats);

    const HOR_RES: usize = 320;
    const VER_RES: usize = 240;
    const BUF_HEIGHT: usize = VER_RES / 20;

    let mut delay: Delay = Default::default();

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    static SCREEN_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();
    let buffer_ref = SCREEN_BUFFER.init([0u8; 512]);
    let di = SpiInterface::new(
        SpiDeviceDriver::new_single(
            peripherals.spi2,
            pins.gpio14,
            pins.gpio13,
            Some(pins.gpio12),
            Some(pins.gpio15),
            &DriverConfig::default().dma(Dma::Auto(buffer_ref.len())),
            &Config::default().baudrate(MegaHertz(40).into()),
        )?,
        PinDriver::output(pins.gpio2)?,
        buffer_ref,
    );

    let mut tft_display = Builder::new(ST7789, di)
        .color_order(mipidsi::options::ColorOrder::Rgb)
        .orientation(
            mipidsi::options::Orientation::default().rotate(mipidsi::options::Rotation::Deg270),
        ) // Mirror on text
        .reset_pin(PinDriver::output(pins.gpio4)?)
        .init(&mut delay)
        .expect("Could not initialize display");

    let touch_clk = pins.gpio25;
    let touch_mosi = pins.gpio32;
    let touch_cs = pins.gpio33;
    let touch_miso = pins.gpio39;

    let touch_driver = SpiDeviceDriver::new_single(
        peripherals.spi3,
        touch_clk,
        touch_mosi,
        Some(touch_miso),
        Some(touch_cs),
        &DriverConfig::new(),
        &Config::new(), //.baudrate(MegaHertz(2).into()).queue_size(3),
    )?;

    let mut touch = Xpt2046::new(touch_driver, None);

    //===========================================================================================================
    //                               Create the User Interface
    //===========================================================================================================

    // Pin 21, Backlight
    let mut bl = PinDriver::output(pins.gpio21)?;
    // Turn on backlight
    bl.set_high()?;

    if !touch.calibrated() {
        // Display is uncalibrated, resolve that before we do anything else.
        let output = touch
            .intrusive_calibration(&mut tft_display, &mut delay)
            .expect("Could not calibrate");
        dbg!(&output);
    }

    let mut display = Display::new(HOR_RES, VER_RES);
    let buffer = DrawBuffer::<{ HOR_RES * BUF_HEIGHT }, Rgb565>::new(HOR_RES, BUF_HEIGHT);
    info!("Display OK");
    display.register(buffer, move |refresh| {
        let area = refresh.rectangle;
        let data = refresh.colors.iter().cloned();

        tft_display
            .fill_contiguous(&area, data)
            .expect("Cannot fill display");

        //refresh.display.flush_ready();
    });

    info!("Draw Buffer OK");

    let mut arc = Arc::new();
    arc.set_size(150, 150);
    arc.set_rotation(135);
    arc.set_bg_angles(0, 270);
    arc.set_value(10);
    arc.set_align(Align::Center.into());

    let mut label = Label::new();
    label.set_long_mode(LabelLongMode::Clip.into());
    label.set_text_static(c"asdasdasd");
    label.set_align(Align::TopMid.into());

    arc.add_event_cb(EventCode::ValueChanged, move |mut event| {
        let Some(obj) = event.get_target_obj() else {
            warn!("Target obj was null");
            return;
        };
        let value = obj.downcast::<Arc<Wdg>>().unwrap().get_value();
        let text = CString::new(value.to_string()).unwrap();
        label.set_text(text.as_c_str());
    });

    info!("Widgets OK");

    let _pointer = InputDevice::<Pointer>::new(|| {
        let event = touch.get_touch_event();
        if let Err(error) = event {
            error!("{}", error)
        }
        get_touch_input(event.ok().flatten())
    });

    info!("Pointer OK");

    unsafe {
        lv_tick_set_cb(Some(xTaskGetTickCount));
    }
    let mut tick = unsafe { xTaskGetTickCount() };

    loop {
        let delay = lv_timer_handler();
        match delay {
            NextTimerPeriod::Ready => {
                continue;
            }
            NextTimerPeriod::AfterMs(delay) => unsafe {
                esp_idf_svc::sys::xTaskDelayUntil(&mut tick, delay.get());
            },
            NextTimerPeriod::Never => unsafe {
                esp_idf_svc::sys::vTaskDelay(LV_DEF_REFR_PERIOD);
            },
        }
    }
}

fn get_touch_input(event: Option<TouchEvent>) -> InputEvent<Pointer> {
    static IS_POINTER_DOWN: AtomicBool = AtomicBool::new(false);
    static LATEST_TOUCH_STATUS: Mutex<InputEvent<Pointer>> =
        Mutex::new(InputEvent::new(Point::zero()));

    let Some(event) = event else {
        return *LATEST_TOUCH_STATUS.lock().unwrap();
    };

    let mut next_touch_status = None;

    match event.kind {
        TouchKind::Start => {
            next_touch_status = Some(InputEvent {
                status: BufferStatus::Once,
                state: InputState::Pressed,
                data: event.point,
            });
            IS_POINTER_DOWN.store(true, Ordering::Relaxed);
        }
        TouchKind::Move => {
            if IS_POINTER_DOWN.load(Ordering::Relaxed) {
                next_touch_status = Some(InputEvent {
                    status: BufferStatus::Once,
                    state: InputState::Pressed,
                    data: event.point,
                });
            }
        }
        TouchKind::End => {
            next_touch_status = Some(InputEvent {
                status: BufferStatus::Once,
                state: InputState::Released,
                data: Point::new(0, 0),
            });
            IS_POINTER_DOWN.store(false, Ordering::Relaxed);
        }
    }
    let mut lock = LATEST_TOUCH_STATUS.lock().unwrap();

    if let Some(latest_touch_status) = next_touch_status {
        *lock = latest_touch_status;
    }
    return *lock;
}
