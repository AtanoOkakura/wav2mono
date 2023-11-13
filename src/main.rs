use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use wav2mono::Wav;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("please specify input dir or file");
        return Ok(());
    }
    let input_path = PathBuf::from(&args[1]);
    let input_dir = get_input_dir(input_path.to_owned()).unwrap();

    for f in fs::read_dir(input_dir.clone())? {
        let file = f?.path();
        let output_path = input_dir.join("mono").join(file.file_name().unwrap());
        Wav::open(&file).to_mono().write(&output_path)?;
    }
    Ok(())
}

fn get_input_dir(path: PathBuf) -> Option<PathBuf> {
    if path.is_dir() {
        Some(path)
    } else {
        path.parent().map(|p| p.to_owned())
    }
}
