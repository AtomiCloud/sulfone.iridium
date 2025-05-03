use flate2::read::GzDecoder;
use std::error::Error;
use std::io::Read;
use tar::Archive;

use super::traits::FileUnpacker;
use super::VirtualFileSystem;

// TarGzUnpacker implementation for tar.gz archives
pub struct TarGzUnpacker;

impl FileUnpacker for TarGzUnpacker {
    fn unpack(&self, archive_data: Vec<u8>) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        let tar_gz = GzDecoder::new(&archive_data[..]);
        let mut archive = Archive::new(tar_gz);
        let mut vfs = VirtualFileSystem::new();

        for entry in archive
            .entries()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
        {
            let mut entry = entry.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            let path = entry
                .path()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
                .to_path_buf();

            // Skip directories - we only want to process files
            if entry.header().entry_type().is_dir() {
                continue;
            }

            let mut buffer = Vec::new();
            entry
                .read_to_end(&mut buffer)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            vfs.add_file(path, buffer);
        }

        Ok(vfs)
    }
}
