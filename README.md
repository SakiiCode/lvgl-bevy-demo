# lvgl-bevy-demo

This is a demo project for [lv_bevy_ecs](https://github.com/SakiiCode/lv_bevy_ecs)

Tested with ESP32 only.

### Installing the toolchain

```sh
cargo install espup --locked
espup install
cargo install espflash cargo-espflash ldproxy
```

### Building

You need additional env variables in `.cargo/config-local.toml` and the PATH applied from ~/export-esp.sh

```toml
[env]
LIBCLANG_PATH = '...'
BINDGEN_EXTRA_CLANG_ARGS = '--sysroot ...'
LV_COMPILE_ARGS='-I%USERPROFILE%\.rustup\toolchains\esp\xtensa-esp-elf\xtensa-esp-elf\include' # Windows only
```

`LIBCLANG_PATH` can be found in ~/export-esp.sh

`BINDGEN_EXTRA_CLANG_ARGS` sysroot can be found with `xtensa-esp32-elf-ld --print-sysroot`

### Flashing

```sh
cargo espflash flash --monitor
```

### Partitions

It can happen that the project does not fit in the default main partition. To fix that you need to generate a partitions.csv with

```sh
cargo espflash partition-table -o partitions.csv --to-csv target/xtensa-esp32-espidf/release/partition-table.bin
```

and increase the `factory` partition size.

Then add this to `espflash.toml`:

```toml
[idf]
partition_table = "partitions.csv"
```

### Upload speed

To increase upload speed set `baudrate = 460800` in `espflash.toml`
