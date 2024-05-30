use crate::{config::Config, context::ConfigContext};
use std::{fs, io, path::Path};
use zip::ZipWriter;
use zip_extensions::ZipWriterExtensions;

pub fn save_backup() {
    let Config {
        server: server_conf,
        backup: backup_conf,
        ..
    } = ConfigContext::get();

    let Some(backup_conf) = backup_conf else {
        return;
    };

    let server_dir = Path::new(&server_conf.work_dir);
    let output_dir = Path::new(&backup_conf.output_dir);

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
