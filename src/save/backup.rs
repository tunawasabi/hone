use crate::config::{Backup, Server};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};
use walkdir::WalkDir;
use zip::{self, ZipWriter};

pub fn save_backup(backup_config: Option<Backup>, server_config: Server) {
    let Some(config) = backup_config else {
        return;
    };

    let output_dir = config.output_dir;
    let save_dir = server_config.work_dir;

    read_save_and_write(save_dir, output_dir).unwrap();
}

fn read_save_and_write(save_dir: String, output_dir: String) -> io::Result<()> {
    let output_dir = Path::new(&output_dir);

    if !output_dir.exists() {
        io::Error::new(
            io::ErrorKind::NotFound,
            "指定されたバックアップディレクトリが存在しません",
        );
    }

    let now = chrono::Local::now();
    let output_dir = output_dir.join(now.format("%Y-%m-%d_%H-%M-%S").to_string());

    let save_dir = Path::new(&save_dir).join("world");
    let save_dir = save_dir.canonicalize()?;

    let mut zip = ZipWriter::new(fs::File::create(output_dir.with_extension("zip"))?);
    let mut buffer = Vec::new();

    for entry in WalkDir::new(&save_dir) {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let path = path.strip_prefix(&save_dir).unwrap();
            zip.start_file(path.to_str().unwrap(), Default::default())?;
            let mut file = fs::File::open(path)?;
            file.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        }
    }

    zip.finish()?;

    Ok(())
}
