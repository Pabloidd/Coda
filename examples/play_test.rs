use std::fs::File;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("audio path");
    let stream = rodio::OutputStreamBuilder::open_default_stream()?;
    let sink = rodio::Sink::connect_new(stream.mixer());
    let src = rodio::Decoder::try_from(File::open(&path)?)?;
    println!("decode ok, playing 3s… empty={}", sink.empty());
    sink.append(src);
    sink.set_volume(0.4);
    for i in 0..15 {
        println!("[{i}] empty={} paused={}", sink.empty(), sink.is_paused());
        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}
