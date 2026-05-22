# lvgl-bevy-demo

This is a `no_std` demo project for [lv_bevy_ecs](https://github.com/SakiiCode/lv_bevy_ecs)

Tested with ESP32 only.

If `std` environment is needed, check out the [esp-idf-svc](https://github.com/SakiiCode/lvgl-bevy-demo/tree/esp-idf-svc) branch or the [lvgl-bevy-demo-dsi](https://github.com/SakiiCode/lvgl-bevy-demo-dsi) project.

### Installing the toolchain

```sh
cargo install espup espflash ldproxy
espup install
```

### Building

You need additional env variables in `.cargo/config-local.toml` and the PATH applied from ~/export-esp.sh

```toml
[env]
LIBCLANG_PATH = '...'
BINDGEN_EXTRA_CLANG_ARGS = '--sysroot ...'
LV_COMPILE_ARGS='-I%USERPROFILE%\.rustup\toolchains\esp\xtensa-esp-elf\xtensa-esp-elf\include' # Windows only!
```

`LIBCLANG_PATH` can be found in ~/export-esp.sh

`BINDGEN_EXTRA_CLANG_ARGS` sysroot can be found with `xtensa-esp32-elf-ld --print-sysroot`

### Flashing

```sh
cargo run
```

### Upload speed

To increase upload speed set `baudrate = 460800` in `espflash.toml`
