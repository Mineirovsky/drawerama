mod app;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let path = match drawerama::storage::database_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("drawerama: {err:#}");
            std::process::exit(1);
        }
    };
    let store = match drawerama::storage::Store::open(&path) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("drawerama: {err:#}");
            std::process::exit(1);
        }
    };
    let snapshot = match store.load() {
        Ok(snapshot) => snapshot,
        Err(err) => {
            eprintln!("drawerama: {err:#}");
            std::process::exit(1);
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 740.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Drawerama",
        options,
        Box::new(move |_cc| Ok(Box::new(app::DraweramaApp::new(store, snapshot, path)))),
    )
}
