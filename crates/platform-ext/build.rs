use std::{env, fs, path::PathBuf};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    let winmd_dir = manifest_dir.join("winmd");
    let out_file = out_dir.join("windows_ai_bindings.rs");

    let inputs = [
        winmd_dir.join("Microsoft.Graphics.winmd"),
        winmd_dir.join("Microsoft.Graphics.Imaging.winmd"),
        winmd_dir.join("Microsoft.Windows.AI.winmd"),
        winmd_dir.join("Microsoft.Windows.AI.Foundation.winmd"),
        winmd_dir.join("Microsoft.Windows.AI.Text.winmd"),
        winmd_dir.join("Microsoft.Windows.AI.Imaging.winmd"),
    ];
    for input in &inputs {
        println!("cargo:rerun-if-changed={}", input.display());
    }

    let out_file_string = out_file.to_string_lossy().into_owned();
    let args = vec![
        "--in".to_string(),
        "default".to_string(),
        "--in".to_string(),
        inputs[0].to_string_lossy().into_owned(),
        "--in".to_string(),
        inputs[1].to_string_lossy().into_owned(),
        "--in".to_string(),
        inputs[2].to_string_lossy().into_owned(),
        "--in".to_string(),
        inputs[3].to_string_lossy().into_owned(),
        "--in".to_string(),
        inputs[4].to_string_lossy().into_owned(),
        "--in".to_string(),
        inputs[5].to_string_lossy().into_owned(),
        "--out".to_string(),
        out_file_string,
        "--flat".to_string(),
        "--filter".to_string(),
        "Microsoft.Windows.AI.Imaging.TextRecognizer".to_string(),
        "--filter".to_string(),
        "Microsoft.Windows.AI.Imaging.RecognizedText*".to_string(),
        "--filter".to_string(),
        "Microsoft.Windows.AI.Imaging.RecognizedLine*".to_string(),
        "--filter".to_string(),
        "Microsoft.Windows.AI.Imaging.RecognizedWord*".to_string(),
        "--filter".to_string(),
        "Microsoft.Windows.AI.AIFeatureReady*".to_string(),
        "--filter".to_string(),
        "Microsoft.Graphics.Imaging.ImageBuffer".to_string(),
        "--filter".to_string(),
        "Microsoft.Graphics.Imaging.ImageBufferPixelFormat".to_string(),
        "--filter".to_string(),
        "Windows.Storage.Streams.IBuffer".to_string(),
        "--filter".to_string(),
        "Windows.Graphics.Imaging.SoftwareBitmap".to_string(),
        "--no-comment".to_string(),
    ];

    let warnings = windows_bindgen::bindgen(args);
    fs::write(out_dir.join("windows_ai_warnings.txt"), warnings.to_string())
        .expect("write windows ai warnings");
    let generated = fs::read_to_string(&out_file).expect("read generated windows ai bindings");
    let generated = generated.replacen(
        "#![allow(\n    non_snake_case,\n    non_upper_case_globals,\n    non_camel_case_types,\n    dead_code,\n    clippy::all\n)]\n\n",
        "",
        1,
    );
    fs::write(&out_file, generated).expect("rewrite generated windows ai bindings");
}
