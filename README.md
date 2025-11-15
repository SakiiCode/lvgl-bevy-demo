# lvgl-bevy-demo

This is a demo project for [lv_bevy_ecs](https://github.com/SakiiCode/lv_bevy_ecs)

Tested with ESP32 only.

You need four env variables in config.toml and the PATH applied from ~/export-esp.sh

```
DEP_LV_CONFIG_PATH = { relative = true, value = "." }
LIBCLANG_PATH = "..."
CROSS_COMPILE = "xtensa-esp32-elf"
BINDGEN_EXTRA_CLANG_ARGS = "--sysroot ..."
```

`LIBCLANG_PATH` can be found in ~/export-esp.sh

`BINDGEN_EXTRA_CLANG_ARGS` sysroot can be found with `xtensa-esp32-elf-ld --print-sysroot`

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
