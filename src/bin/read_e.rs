use std::fs::File;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let volume_path = r"\\?\Volume{8f4c1f1b-7663-4821-8248-a2fe2206ddad}";
    println!("Opening volume {}...", volume_path);
    
    let mut file = File::open(volume_path)?;
    let mut sector = vec![0u8; 512];
    file.read_exact(&mut sector)?;
    
    println!("First 64 bytes of volume E::");
    println!("{:X?}", &sector[..64]);
    
    let oem_id = String::from_utf8_lossy(&sector[3..11]);
    println!("OEM ID: {:?}", oem_id);
    
    if &sector[3..11] == b"NTFS    " {
        println!("NTFS Signature matches!");
    } else {
        println!("NTFS Signature DOES NOT match!");
    }
    
    Ok(())
}
