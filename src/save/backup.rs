use crate::config::{Backup, Server};
use std::{fs, io, path::Path};
use zip::ZipWriter;
use zip_extensions::ZipWriterExtensions;

pub fn save_backup(backup_config: Option<Backup>, server_config: Server) {
    let Some(config) = backup_config else {
        return;
    };

    let server_dir = Path::new(&server_config.work_dir);
    let output_dir = Path::new(&config.output_dir);

    read_save_and_write(server_dir, output_dir).unwrap();
}

fn read_save_and_write(server_dir: &Path, output_dir: &Path) -> io::Result<()> {
    if !output_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "指定されたバックアップフォルダが存在しません",
        ));
    };

    let world_dir = Path::new(&server_dir).join("world");

    let now = chrono::Local::now();
    let filename = format!("world_backup_{}", now.format("%Y%m%d_%H%M%S"));

    // Create a backup archive
    let zip = output_dir.join(filename).with_extension("zip");
    let mut zip = ZipWriter::new(fs::File::create(zip)?);
    if let Err(e) = zip.create_from_directory(&world_dir) {
        return match e {
            zip::result::ZipError::Io(err) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("zipファイルを作成できませんでした: {err}"),
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("zipファイルを作成できませんでした: {e}"),
            )),
        };
    };

    Ok(())
}
