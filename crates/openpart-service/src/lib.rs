use anyhow::Result;
use openpart_core::DiskState;

pub fn scan_disks() -> Result<DiskState> {
    openpart_core::scanner::scan_disks().map_err(|e| anyhow::anyhow!("Scanner error: {:?}", e))
}

pub fn run() -> Result<()> {
    let state = scan_disks()?;
    println!("{:#?}", state);
    Ok(())
}
