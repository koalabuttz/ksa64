fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target != "armv7-sony-vita-newlibeabihf" {
        return;
    }
    let sdk = std::env::var("VITASDK").expect("VITASDK must name the pinned VitaSDK");
    println!("cargo:rustc-link-search=native={sdk}/arm-vita-eabi/lib");
    println!("cargo:rustc-link-lib=static=SDL2");
    for library in [
        "SceGxm_stub",
        "SceDisplay_stub",
        "SceCtrl_stub",
        "SceAppMgr_stub",
        "SceAppUtil_stub",
        "SceAudio_stub",
        "SceAudioIn_stub",
        "SceSysmodule_stub",
        "SceIofilemgr_stub",
        "SceCommonDialog_stub",
        "SceTouch_stub",
        "SceHid_stub",
        "SceMotion_stub",
        "ScePower_stub",
        "SceNet_stub",
        "SceLibKernel_stub",
        "SceProcessmgr_stub",
    ] {
        println!("cargo:rustc-link-lib={library}");
    }
    println!("cargo:rustc-link-lib=m");
}
