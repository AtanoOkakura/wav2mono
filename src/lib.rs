use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use wav::bit_depth::BitDepth;
use wav::header::Header;

pub fn wav_files_to_mono(dir: &str) -> io::Result<()> {
    for f in fs::read_dir(dir)? {
        let f = f?;
        let path = f.path();
        if path.extension().unwrap_or_default() == "wav" {
            wav_file_to_mono(&path)?
        }
    }
    Ok(())
}

pub fn wav_file_to_mono(path: &Path) -> io::Result<()> {
    let (header, data) = open_wav(path)?;
    let (header, data) = to_mono(header, data)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to convert to mono"))?;
    write_wav(path, header, data)
}

pub fn open_wav(path: &Path) -> io::Result<(Header, BitDepth)> {
    let mut input_file = File::open(&path)?;
    wav::read(&mut input_file)
}

pub fn write_wav(path: &Path, header: Header, data: BitDepth) -> io::Result<()> {
    let mut output_file = File::create(path)?;
    wav::write(header, &data, &mut output_file)
}

pub fn to_mono(header: Header, data: BitDepth) -> Option<(Header, BitDepth)> {
    if data.is_empty() {
        None
    } else {
        let channel_count = header.channel_count;
        let new_header = Header {
            channel_count: 1,
            ..header
        };
        let new_data = match data {
            BitDepth::Eight(d) => BitDepth::Eight(to_mono_data(d.clone(), channel_count)),
            BitDepth::Sixteen(d) => BitDepth::Sixteen(to_mono_data(d.clone(), channel_count)),
            BitDepth::TwentyFour(d) => BitDepth::TwentyFour(to_mono_data(d.clone(), channel_count)),
            BitDepth::ThirtyTwoFloat(d) => {
                BitDepth::ThirtyTwoFloat(to_mono_data(d.clone(), channel_count))
            }
            _ => unreachable!(),
        };
        Some((new_header, new_data))
    }
}

fn to_mono_data<Int>(data: Vec<Int>, channels_count: u16) -> Vec<Int>
where
    Int: Clone,
{
    data.chunks(channels_count as usize)
        .map(|chunk| chunk[0].clone())
        .collect()
}

pub struct Wav {
    header: Header,
    data: BitDepth,
}

impl Wav {
    pub fn new(header: Header, data: BitDepth) -> Self {
        Wav { header, data }
    }

    pub fn open(path: &Path) -> Self {
        let (h, d) = open_wav(path).unwrap_or_else(|_| panic!("Can't open {:?}", path));
        Wav::new(h, d)
    }

    pub fn write(&self, path: &Path) -> io::Result<()> {
        //create directory if missing
        let dir = path.parent().unwrap();
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        //write wav file
        write_wav(path, self.header, self.data.clone())
    }

    pub fn to_mono(&mut self) -> &mut Wav {
        let (h, d) = to_mono(self.header, self.data.clone()).unwrap();
        self.header = h;
        self.data = d;
        self
    }
}

//test
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    //test open wave file
    #[test]
    fn test_open_wav() {
        let (header, data) = open_wav(Path::new("test/test.wav")).unwrap();
        println!("{:?}", header);
        println!("{}", data.is_eight());
        assert_eq!(header.channel_count, 2);
        assert!(data.is_sixteen());
    }

    #[test]
    fn test_path() {
        let path = Path::new("test/mono/test.wav");
        println!("Path is: {:?}", path);
        assert!(path.is_relative());
        assert!(path.exists());
        assert_eq!(path.parent().unwrap().to_str().unwrap(), "test/mono");
    }

    #[test]
    fn test_wav_to_mono() {
        let mut wav = Wav::open(Path::new("test/test.wav"));
        wav.to_mono();
        assert!(wav.write(Path::new("test/mono/test.wav")).is_ok());
    }
}
