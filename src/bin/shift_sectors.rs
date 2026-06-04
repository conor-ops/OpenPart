use std::fs::OpenOptions;
use std::io::{Read, Write, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = r"\\.\PhysicalDrive0";
    let src_offset: u64 = 244_350_541_824;
    let dest_offset: u64 = 244_349_665_280;
    let size_to_copy: u64 = 11_709_448_192; // Copy the full size

    println!("Opening disk {}", disk_path);
    let mut file_read = OpenOptions::new()
        .read(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    let mut file_write = OpenOptions::new()
        .write(true)
        .share_mode(1 | 2)
        .open(disk_path)?;

    let chunk_size: usize = 64 * 1024 * 1024; // 64 MB
    let mut raw_buf = vec![0u8; chunk_size + 4096];
    let ptr = raw_buf.as_ptr() as usize;
    let align_offset = (4096 - (ptr % 4096)) % 4096;
    let buf = &mut raw_buf[align_offset .. align_offset + chunk_size];

    let total_chunks = (size_to_copy as f64 / chunk_size as f64).ceil() as u64;
    println!("Starting shift of {} bytes ({} chunks) from {} to {}...", size_to_copy, total_chunks, src_offset, dest_offset);

    for i in 0..total_chunks {
        let chunk_src = src_offset + (i * chunk_size as u64);
        let chunk_dest = dest_offset + (i * chunk_size as u64);
        let bytes_this_chunk = std::cmp::min(chunk_size as u64, size_to_copy - (i * chunk_size as u64)) as usize;

        file_read.seek(SeekFrom::Start(chunk_src))?;
        file_read.read_exact(&mut buf[..bytes_this_chunk])?;

        file_write.seek(SeekFrom::Start(chunk_dest))?;
        file_write.write_all(&buf[..bytes_this_chunk])?;
        file_write.flush()?;

        if i % 10 == 0 || i == total_chunks - 1 {
            let done = (i + 1) * chunk_size as u64;
            let pct = (done as f64 / size_to_copy as f64 * 100.0).round();
            println!("Copied {:.2} GB / {:.2} GB ({}%)", done as f64 / 1_073_741_824.0, size_to_copy as f64 / 1_073_741_824.0, pct);
        }
    }

    println!("Shift complete!");
    Ok(())
}
