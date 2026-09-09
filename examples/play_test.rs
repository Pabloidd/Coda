use std::fs::File;
use std::io::BufReader;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("mp3 path");
    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;
    let src = rodio::Decoder::new(BufReader::new(File::open(&path)?))?;
    println!("decode ok, playing 3s… empty={}", sink.empty());
    sink.append(src);
    sink.set_volume(0.4);
    for i in 0..15 {
        println!("[{i}] empty={} paused={}", sink.empty(), sink.is_paused());
        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}
