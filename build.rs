fn main() {
    // these env variables must be set based on your installation
    // put them into .cargo/config.toml
    // https://github.com/SakiiCode/lv_bevy_ecs?tab=readme-ov-file#building-for-embedded
    let _libclang_path = env!("LIBCLANG_PATH");
    let _bindgen_extra_clang_args = env!("BINDGEN_EXTRA_CLANG_ARGS");
    embuild::espidf::sysenv::output();
}
