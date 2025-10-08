use std::ffi::CString;

use anyhow::Result;
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565, prelude::Point};
use esp_idf_svc::hal::{
    delay::{Delay, FreeRtos},
    gpio::PinDriver,
    prelude::Peripherals,
    spi::{
        config::{Config, DriverConfig},
        Dma, SpiDeviceDriver,
    },
    units::MegaHertz,
};
use log::info;
use lv_bevy_ecs::{
    display::{Display, DrawBuffer},
    events::Event,
    functions::lv_log_init,
    input::{BufferStatus, InputDevice, InputEvent, InputState, Pointer},
    prelude::*,
    support::LabelLongMode,
    widgets::{Arc, Label},
    LvglSchedule, LvglWorld,
};
use mipidsi::{interface::SpiInterface, models::ST7789, Builder};
use static_cell::StaticCell;
use xpt2046::{TouchKind, TouchScreen, Xpt2046};

static SCREEN_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    //esp_idf_svc::log::EspLogger::initialize_default();

    // Use LVGL logger instead
    lv_log_init();

    let mut delay: Delay = Default::default();

    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;

    let buffer_ref = SCREEN_BUFFER.init([0u8; 256]);
    let di = SpiInterface::new(
        SpiDeviceDriver::new_single(
            peripherals.spi2,
            pins.gpio14,
            pins.gpio13,
            Some(pins.gpio12),
            Some(pins.gpio15),
            &DriverConfig::new().dma(Dma::Disabled),
            &Config::new().baudrate(MegaHertz(40).into()),
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

    const HOR_RES: u32 = 320;
    const VER_RES: u32 = 240;
    const LINE_HEIGHT: u32 = 10;

    // Pin 21, Backlight
    let mut bl = PinDriver::output(pins.gpio21)?;
    // Turn on backlight
    bl.set_high()?;
    if !touch.calibrated() {
        // Display is uncalibrated, resolve that before we do anything else.
        let output = touch
            .intrusive_calibration(&mut tft_display, &mut delay)
            .expect("Cannot calibrate");
        dbg!(&output);
    }

    let mut display = Display::create(HOR_RES as i32, VER_RES as i32);
    let buffer =
        DrawBuffer::<{ (HOR_RES * LINE_HEIGHT) as usize }, Rgb565>::create(HOR_RES, LINE_HEIGHT);
    info!("Display OK");
    display.register(buffer, |refresh| {
        let area = refresh.rectangle;
        let data = refresh.colors.iter().cloned();

        tft_display
            .fill_contiguous(&area, data)
            .expect("Cannot fill display");
    });

    info!("Draw Buffer OK");

    let mut world = LvglWorld::new();
    //world.add_observer(on_insert_children);

    info!("World OK");

    // Create screen and widgets
    //let mut screen: lvgl::Screen = display.get_scr_act().map_err(BoardError::DISPLAY)?;

    // let mut screen_style = Style::default();
    // screen_style.set_bg_color(Color::from_rgb((100, 100, 100)));
    // screen.add_style(Part::Main, &mut screen_style);

    let arc = Arc::create_widget()?;
    unsafe {
        lv_obj_set_size(arc.raw(), 150, 150);
        lv_arc_set_rotation(arc.raw(), 135);
        lv_arc_set_bg_angles(arc.raw(), 0, 270);
        lv_arc_set_value(arc.raw(), 10);
        lv_obj_set_align(arc.raw(), lv_align_t_LV_ALIGN_CENTER);
    }

    let label = Label::create_widget()?;
    unsafe {
        lv_label_set_long_mode(label.raw(), LabelLongMode::Dots.into());
        lv_label_set_text(label.raw(), c"asdasdasd".to_bytes_with_nul().as_ptr());
        lv_obj_set_align(label.raw(), lv_align_t_LV_ALIGN_TOP_MID);
    }
    lv_bevy_ecs::events::lv_obj_add_event_cb(&arc, Event::ValueChanged, |mut event| unsafe {
        let target = lv_event_get_target_obj(&mut event);
        let value = lv_arc_get_value(target);
        lv_label_set_text(
            label.raw(),
            CString::new(value.to_string())
                .unwrap()
                .as_bytes_with_nul()
                .as_ptr(),
        );
    });

    world.spawn((Label, label));
    world.spawn((Arc, arc));

    info!("Widgets OK");

    //let mut latest_touch_status = PointerInputData::Touch(Point::new(0, 0)).released().once();
    let mut latest_touch_status = InputEvent {
        status: BufferStatus::Once,
        state: InputState::Released,
        data: Point::new(0, 0),
    };

    let _pointer = InputDevice::<Pointer>::create(|| latest_touch_status);

    info!("Pointer OK");

    let mut is_pointer_down = false;

    let mut schedule = LvglSchedule::new();

    FreeRtos::delay_ms(10);
    info!("Sleep OK");

    loop {
        match touch.get_touch_event() {
            Ok(event) => {
                if let Some(event) = event {
                    //dbg!(&event.point);
                    #[allow(unused_assignments)]
                    match event.kind {
                        TouchKind::Start => {
                            //latest_touch_status = PointerInputData::Touch(event.point).pressed().once();
                            latest_touch_status = InputEvent {
                                status: BufferStatus::Once,
                                state: InputState::Pressed,
                                data: event.point,
                            };
                            is_pointer_down = true;
                        }
                        TouchKind::Move => {
                            if is_pointer_down {
                                //latest_touch_status = PointerInputData::Touch(event.point).pressed().once();
                                latest_touch_status = InputEvent {
                                    status: BufferStatus::Once,
                                    state: InputState::Pressed,
                                    data: event.point,
                                };
                            }
                        }
                        TouchKind::End => {
                            //latest_touch_status = PointerInputData::Touch(Point::new(0, 0)).released().once();
                            latest_touch_status = InputEvent {
                                status: BufferStatus::Once,
                                state: InputState::Released,
                                data: Point::new(0, 0),
                            };
                            is_pointer_down = false;
                        }
                    }
                }
            }
            Err(error) => {
                dbg!(error);
            }
        };

        // Run the schedule once. If your app has a "loop", you would run this once per loop
        schedule.run(&mut world);

        unsafe {
            //info!("Tick OK");
            lv_timer_handler();
        }
        //info!("Timer OK");

        FreeRtos::delay_ms(10);
    }
}
