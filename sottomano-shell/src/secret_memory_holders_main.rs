use std::{
    fs,
    io::{self, ErrorKind, Write},
    path::Path,
};

fn main() -> io::Result<()> {
    let process_ids: Vec<u32> = fs::read_dir("/proc")?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
        .collect();
    let mut holder_pids = Vec::new();
    for process_id in process_ids {
        if process_holds_secret_memory(&Path::new("/proc").join(process_id.to_string()))? {
            holder_pids.push(process_id);
        }
    }
    let mut standard_output = io::stdout().lock();
    for holder_pid in holder_pids {
        writeln!(standard_output, "{holder_pid}")?;
    }
    Ok(())
}

fn process_holds_secret_memory(process_directory: &Path) -> io::Result<bool> {
    match fs::read(process_directory.join("maps")) {
        Ok(memory_maps) => Ok(memory_maps
            .split(|byte| *byte == b'\n')
            .any(maps_secret_memory)),
        Err(read_failure) if read_failure.kind() == ErrorKind::PermissionDenied => {
            Err(read_failure)
        }
        Err(_) => Ok(false),
    }
}

fn maps_secret_memory(map_line: &[u8]) -> bool {
    let mapped_path = String::from_utf8_lossy(map_line)
        .split_whitespace()
        .skip(5)
        .collect::<Vec<_>>()
        .join(" ");
    mapped_path.contains("secretmem") && mapped_path.matches('/').count() <= 1
}

#[cfg(test)]
mod tests {
    use super::maps_secret_memory;

    #[test]
    fn a_secret_memory_mapping_is_recognised() {
        assert!(maps_secret_memory(
            b"7f00-7f01 rw-s 00000000 00:01 12345    /secretmem (deleted)"
        ));
        assert!(maps_secret_memory(
            b"7f00-7f01 rw-s 00000000 00:0f 12345    anon_inode:[secretmem]"
        ));
    }

    #[test]
    fn ordinary_files_named_alike_are_not_secret_memory() {
        assert!(!maps_secret_memory(
            b"7f00-7f01 r--p 00000000 103:02 777    /home/user/secretmem"
        ));
        assert!(!maps_secret_memory(
            b"7f00-7f01 r--p 00000000 103:02 777    /nix/store/abc/libfoo.so"
        ));
        assert!(!maps_secret_memory(b"7f00-7f01 rw-p 00000000 00:00 0"));
    }
}
