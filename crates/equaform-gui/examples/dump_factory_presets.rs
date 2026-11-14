use equaform_gui::preset_manager::PresetManager;
use equaform_gui::presets_io::{
    is_factory_preset, populate_factory_presets, sanitize_preset_filename,
};
use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let out_dir = if args.len() > 1 {
        &args[1]
    } else {
        "target/release/staging/presets"
    };

    let out_path = Path::new(out_dir);
    fs::create_dir_all(out_path)?;

    let mut manager = PresetManager::new();
    populate_factory_presets(&mut manager);

    let mut count = 0;
    for named in manager.iter() {
        if !is_factory_preset(&named.name) {
            continue;
        }
        let filename = sanitize_preset_filename(&named.name);
        let file_path = out_path.join(&filename);
        let file_path_str = file_path.to_str().ok_or("Invalid path string")?;
        manager.save_to_file(&named.name, file_path_str)?;
        println!(
            "  - Exported factory preset: {} -> {}",
            named.name, filename
        );
        count += 1;
    }

    println!(
        "Successfully dumped {} factory presets to: {}",
        count,
        out_path.display()
    );
    Ok(())
}
