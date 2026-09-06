use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::names::{LOG_FILE_NAME, MAX_LOG_FILE_BYTES, MAX_LOG_FILES};

pub(crate) struct SizeRollingFile {
    directory: PathBuf,
    file_name: String,
    max_bytes: u64,
    max_files: u32,
    current: Option<File>,
    written: u64,
}

impl SizeRollingFile {
    pub fn open(directory: impl AsRef<Path>) -> io::Result<Self> {
        Self::with_limits(
            directory,
            LOG_FILE_NAME,
            MAX_LOG_FILE_BYTES,
            MAX_LOG_FILES,
        )
    }

    pub(crate) fn with_limits(
        directory: impl AsRef<Path>,
        file_name: &str,
        max_bytes: u64,
        max_files: u32,
    ) -> io::Result<Self> {
        let directory = directory.as_ref().to_path_buf();
        fs::create_dir_all(&directory)?;
        let path = generation_path(&directory, file_name, 0);
        let current = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = current.metadata()?.len();
        Ok(Self {
            directory,
            file_name: file_name.to_owned(),
            max_bytes: max_bytes.max(1),
            max_files: max_files.max(1),
            current: Some(current),
            written,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.current.take() {
            file.flush()?;
            drop(file);
        }

        match self.shift_archives() {
            Ok(()) => self.open_current(true),
            Err(error) => {
                let _ = self.open_current(false);
                Err(error)
            }
        }
    }

    fn shift_archives(&self) -> io::Result<()> {
        let last_generation = self.max_files.saturating_sub(1);
        if last_generation == 0 {
            return Ok(());
        }

        let oldest = generation_path(&self.directory, &self.file_name, last_generation);
        if oldest.exists() {
            fs::remove_file(&oldest)?;
        }
        for generation in (1..last_generation).rev() {
            let from = generation_path(&self.directory, &self.file_name, generation);
            if from.exists() {
                let to = generation_path(&self.directory, &self.file_name, generation + 1);
                fs::rename(&from, &to)?;
            }
        }
        let current_path = generation_path(&self.directory, &self.file_name, 0);
        if current_path.exists() {
            let first_archive = generation_path(&self.directory, &self.file_name, 1);
            fs::rename(&current_path, &first_archive)?;
        }
        Ok(())
    }

    fn open_current(&mut self, truncate: bool) -> io::Result<()> {
        let current_path = generation_path(&self.directory, &self.file_name, 0);
        let mut options = OpenOptions::new();
        options.create(true);
        if truncate {
            options.write(true).truncate(true);
        } else {
            options.append(true);
        }
        let file = options.open(&current_path)?;
        self.written = if truncate {
            0
        } else {
            file.metadata().map(|meta| meta.len()).unwrap_or(0)
        };
        self.current = Some(file);
        Ok(())
    }
}

impl Write for SizeRollingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written > 0 && self.written.saturating_add(buf.len() as u64) > self.max_bytes {
            let _ = self.rotate();
        }
        if self.current.is_none() {
            self.open_current(false)?;
        }
        let file = self.current.as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "log file is not open")
        })?;
        let written = file.write(buf)?;
        self.written = self.written.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.current.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

pub(crate) fn generation_path(directory: &Path, file_name: &str, generation: u32) -> PathBuf {
    if generation == 0 {
        directory.join(file_name)
    } else {
        directory.join(format!("{file_name}.{generation}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "gdom-logs-roll-{}-{}",
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn rotates_at_size_and_keeps_at_most_five_files() {
        let dir = temp_dir();
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp log dir");
        {
            let mut log =
                SizeRollingFile::with_limits(&dir, LOG_FILE_NAME, 32, MAX_LOG_FILES).expect("open");
            for index in 0..12 {
                let line = format!("line-{index:02} ----------------\n");
                log.write_all(line.as_bytes()).expect("write");
            }
            log.flush().expect("flush");
        }

        let mut names: Vec<_> = fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert!(
            names.len() as u32 <= MAX_LOG_FILES,
            "retained {names:?}"
        );
        assert!(names.iter().any(|name| name == LOG_FILE_NAME));
        let first_archive = format!("{LOG_FILE_NAME}.1");
        assert!(names.iter().any(|name| name == &first_archive));
        for name in &names {
            let len = fs::metadata(dir.join(name)).expect("meta").len();
            assert!(len <= 32, "{name} is {len} bytes");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn drops_the_oldest_archive_after_five_files() {
        let dir = temp_dir();
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp log dir");
        {
            let mut log =
                SizeRollingFile::with_limits(&dir, LOG_FILE_NAME, 8, MAX_LOG_FILES).expect("open");
            for _ in 0..20 {
                log.write_all(b"abcdefgh\n").expect("write");
            }
            log.flush().expect("flush");
        }
        let count = fs::read_dir(&dir).expect("read dir").count();
        assert!(count as u32 <= MAX_LOG_FILES);
        assert!(!generation_path(&dir, LOG_FILE_NAME, MAX_LOG_FILES).exists());
        assert!(generation_path(&dir, LOG_FILE_NAME, MAX_LOG_FILES - 1).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
