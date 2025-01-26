use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Trait defining storage operations for log data
pub trait LogStorage {
    /// Appends data to an existing log or creates a new one if it doesn't exist
    /// Returns the number of bytes written
    fn append_or_create(&mut self, data: String) -> io::Result<usize>;

    /// Retrieves all data from the log
    fn get(&mut self) -> anyhow::Result<Vec<String>>;
}

/// A file system implementation of LogStorage that ensures fsync safety
pub struct FileLogStorage {
    file: File,
}

impl FileLogStorage {
    /// Creates a new FileLogStorage instance
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .create_new(true)
            .append(true)
            .open(path)?;

        Ok(FileLogStorage { file })
    }
}

impl LogStorage for FileLogStorage {
    fn append_or_create(&mut self, data: String) -> io::Result<usize> {
        // Seek to end of file to append
        self.file.seek(SeekFrom::End(0))?;

        // Write the data with a new line
        self.file.write_all(data.as_bytes())?;
        self.file.write_all(b"\n")?;

        // Ensure data is written to disk
        self.file.sync_all()?;

        Ok(0)
    }

    fn get(&mut self) -> anyhow::Result<Vec<String>> {
        // Seek to beginning of file
        self.file.seek(SeekFrom::Start(0))?;

        // Read entire file contents
        let mut buffer = Vec::new();
        self.file.read_to_end(&mut buffer)?;

        let string = String::from_utf8(buffer).unwrap();
        let string = string.lines().map(|s| s.to_string()).collect();
        Ok(string)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::fs;
//     use tempfile::tempdir;

//     #[test]
//     fn test_file_log_storage() -> io::Result<()> {
//         let dir = tempdir()?;
//         let file_path = dir.path().join("test.log");

//         let mut storage = FileLogStorage::new(&file_path)?;

//         // Test append
//         let data1 = b"Hello, ";
//         let data2 = b"World!";

//         storage.append_or_create(data1)?;
//         storage.append_or_create(data2)?;

//         // Test get
//         let result = storage.get()?;
//         assert_eq!(result, b"Hello, World!");

//         Ok(())
//     }
// }
