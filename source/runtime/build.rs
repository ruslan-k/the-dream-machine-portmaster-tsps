fn main() {
    // ── Nintendo Switch (libnx) ───────────────────────────────────────────────
    #[cfg(target_os = "horizon")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rustc-link-lib=dylib=nx");
    }

    // ── PlayStation 3 (PSL1GHT) ───────────────────────────────────────────────
    #[cfg(target_os = "ps3")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        // PSL1GHT core libraries
        println!("cargo:rustc-link-lib=static=lv2");
        println!("cargo:rustc-link-lib=static=rsx");
        println!("cargo:rustc-link-lib=static=gcm_sys");
        println!("cargo:rustc-link-lib=static=sysutil");
        println!("cargo:rustc-link-lib=static=audio");
        println!("cargo:rustc-link-lib=static=io");
        println!("cargo:rustc-link-lib=static=net");
        println!("cargo:rustc-link-lib=static=netctl");
        println!("cargo:rustc-link-lib=static=m");
        println!("cargo:rustc-link-lib=static=lv2_stub");
        if let Ok(psl1ght) = std::env::var("PSL1GHT") {
            println!("cargo:rustc-link-search=native={}/target/ppu/lib", psl1ght);
        }
    }

    // ── PlayStation 2 (ps2sdk) ────────────────────────────────────────────────
    #[cfg(target_os = "ps2")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rustc-link-lib=static=ps2sdkc");
        println!("cargo:rustc-link-lib=static=c");
        println!("cargo:rustc-link-lib=static=gcc");
        if let Ok(ps2sdk) = std::env::var("PS2SDK") {
            println!("cargo:rustc-link-search=native={}/ee/lib", ps2sdk);
        }
    }

    // ── Nintendo Wii / GameCube (libogc) ──────────────────────────────────────
    #[cfg(target_os = "wii")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rustc-link-lib=static=ogc");
        println!("cargo:rustc-link-lib=static=m");
        if let Ok(devkitpro) = std::env::var("DEVKITPRO") {
            println!("cargo:rustc-link-search=native={}/libogc/lib/wii", devkitpro);
            println!("cargo:rustc-link-search=native={}/portlibs/wii/lib", devkitpro);
        }
    }

    #[cfg(target_os = "gamecube")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rustc-link-lib=static=ogc");
        println!("cargo:rustc-link-lib=static=m");
        if let Ok(devkitpro) = std::env::var("DEVKITPRO") {
            println!("cargo:rustc-link-search=native={}/libogc/lib/cube", devkitpro);
        }
    }

    // ── Nintendo Wii U (wut) ─────────────────────────────────────────────────
    #[cfg(target_os = "wiiu")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rustc-link-lib=static=wut");
        println!("cargo:rustc-link-lib=static=m");
        if let Ok(devkitpro) = std::env::var("DEVKITPRO") {
            println!("cargo:rustc-link-search=native={}/wut/lib", devkitpro);
            println!("cargo:rustc-link-search=native={}/portlibs/wiiu/lib", devkitpro);
        }
    }

    // ── Xbox (OG) / Xbox 360 (nxdk / libxenon) ───────────────────────────────
    #[cfg(target_os = "xbox")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        // nxdk provides SDL (v1) and pbgl (OpenGL 1.x compat)
        println!("cargo:rustc-link-lib=static=SDL");
        println!("cargo:rustc-link-lib=static=pbgl");
        println!("cargo:rustc-link-lib=static=pbkit");
        println!("cargo:rustc-link-lib=static=winapi");
        println!("cargo:rustc-link-lib=static=hal");
        println!("cargo:rustc-link-lib=static=xboxkrnl");
        println!("cargo:rustc-link-lib=static=m");
        println!("cargo:rustc-link-lib=static=c");
        if let Ok(nxdk) = std::env::var("NXDK_DIR") {
            println!("cargo:rustc-link-search=native={}/lib", nxdk);
        }
    }

    #[cfg(target_os = "xbox360")]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rustc-link-lib=static=xenon");
        println!("cargo:rustc-link-lib=static=m");
        println!("cargo:rustc-link-lib=static=c");
        if let Ok(xenon) = std::env::var("LIBXENON_DIR") {
            println!("cargo:rustc-link-search=native={}/lib", xenon);
        }
    }
}
