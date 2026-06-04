fn main() {
    if cfg!(target_os = "windows") {
        let mut res = tauri_winres::WindowsResource::new();
        res.set_icon("../../icons/icon.ico");
        res.compile().unwrap();
    }
}
