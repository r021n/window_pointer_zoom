fn main() {
    println!("cargo:rerun-if-env-changed=WPZ_UIACCESS");
    println!("cargo:rerun-if-changed=uiaccess.manifest.xml");

    let uiaccess = match std::env::var("WPZ_UIACCESS").as_deref() {
        Ok("1") => true,
        Ok("0") | Err(_) => false,
        Ok(_) => panic!("WPZ_UIACCESS must be 0 or 1"),
    };
    let attributes = if uiaccess {
        assert_eq!(
            std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
            Ok("windows"),
            "UIAccess builds require a Windows target"
        );
        tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new()
                .app_manifest(include_str!("uiaccess.manifest.xml")),
        )
    } else {
        tauri_build::Attributes::new()
    };
    tauri_build::try_build(attributes).expect("failed to build Tauri resources");
}
