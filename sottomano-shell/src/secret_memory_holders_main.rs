use std::{
    fs,
    io::{self, ErrorKind, Write},
    path::Path,
};

fn main() -> io::Result<()> {
    let process_ids: Vec<u32> = fs::read_dir("/proc")?
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|entry| {
            let directory_name = entry.file_name();
            let directory_text = directory_name.to_str()?;
            directory_text
                .bytes()
                .all(|byte| byte.is_ascii_digit())
                .then(|| {
                    directory_text
                        .parse()
                        .expect("decimal /proc directory names are process ids")
                })
        })
        .collect();
    let mut holder_pids = Vec::new();
    for process_id in process_ids {
        if process_mapping_kind(&Path::new("/proc").join(process_id.to_string()))?
            == MappingKind::SecretMemory
        {
            holder_pids.push(process_id);
        }
    }
    let mut standard_output = io::stdout().lock();
    for holder_pid in holder_pids {
        writeln!(standard_output, "{holder_pid}")?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MappingKind {
    SecretMemory,
    Ordinary,
}

fn process_mapping_kind(process_directory: &Path) -> io::Result<MappingKind> {
    match fs::read(process_directory.join("maps")) {
        Ok(memory_maps) => Ok(
            if memory_maps
                .split(|byte| *byte == b'\n')
                .any(|map_line| mapping_kind(map_line) == MappingKind::SecretMemory)
            {
                MappingKind::SecretMemory
            } else {
                MappingKind::Ordinary
            },
        ),
        Err(read_failure) if read_failure.kind() == ErrorKind::PermissionDenied => {
            Err(read_failure)
        }
        Err(_) => Ok(MappingKind::Ordinary),
    }
}

fn mapping_kind(map_line: &[u8]) -> MappingKind {
    let mapped_path = String::from_utf8_lossy(map_line)
        .split_whitespace()
        .skip(5)
        .collect::<Vec<_>>()
        .join(" ");
    if mapped_path.contains("secretmem") && mapped_path.matches('/').count() <= 1 {
        MappingKind::SecretMemory
    } else {
        MappingKind::Ordinary
    }
}

#[cfg(test)]
mod tests {
    use super::{MappingKind, mapping_kind};

    #[test]
    fn a_secret_memory_mapping_is_recognised() {
        assert_eq!(
            mapping_kind(b"7f00-7f01 rw-s 00000000 00:01 12345    /secretmem (deleted)"),
            MappingKind::SecretMemory
        );
        assert_eq!(
            mapping_kind(b"7f00-7f01 rw-s 00000000 00:0f 12345    anon_inode:[secretmem]"),
            MappingKind::SecretMemory
        );
    }

    #[test]
    fn ordinary_files_named_alike_are_not_secret_memory() {
        assert_eq!(
            mapping_kind(b"7f00-7f01 r--p 00000000 103:02 777    /home/user/secretmem"),
            MappingKind::Ordinary
        );
        assert_eq!(
            mapping_kind(b"7f00-7f01 r--p 00000000 103:02 777    /nix/store/abc/libfoo.so"),
            MappingKind::Ordinary
        );
        assert_eq!(
            mapping_kind(b"7f00-7f01 rw-p 00000000 00:00 0"),
            MappingKind::Ordinary
        );
    }
}
