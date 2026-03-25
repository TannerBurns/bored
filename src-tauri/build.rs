fn main() {
    #[cfg(target_os = "macos")]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let plist_path = std::path::Path::new(&out_dir).join("Info.plist");
        std::fs::write(
            &plist_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.bored.app</string>
    <key>CFBundleName</key>
    <string>Bored</string>
    <key>NSMicrophoneUsageDescription</key>
    <string>Bored uses the microphone for dictation input in text fields.</string>
    <key>NSSpeechRecognitionUsageDescription</key>
    <string>Bored uses speech recognition for dictation input in text fields.</string>
</dict>
</plist>"#,
        )
        .expect("Failed to write Info.plist");

        println!("cargo:rustc-link-arg=-sectcreate");
        println!("cargo:rustc-link-arg=__TEXT");
        println!("cargo:rustc-link-arg=__info_plist");
        println!("cargo:rustc-link-arg={}", plist_path.display());
    }

    tauri_build::build()
}
