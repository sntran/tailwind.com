//! Make and read the patch files of the packages of Tailwind CSS.
//!
//! Usage:
//!   twpatch pack PACKAGES OUT VERSION...
//!   twpatch unpack STORE VERSION OUT
//!
//! `pack` reads the packages PACKAGES/VERSION (the files of the npm
//! package `tailwindcss`: scripts/packages.sh makes them) and writes a
//! store to OUT: the newest version as a directory with its files, and
//! each older version as OUT/VERSION.patch, the differences from the next
//! newer version. The versions go from old to new in the arguments. The
//! tool then reads each version back from the store and compares it with
//! the package: a patch that gives another file is an error.
//!
//! `unpack` writes the files of one version of a store to the directory
//! OUT.
//!
//! scripts/build.sh puts the store into the ZIP directory `tailwindcss/`
//! of tailwind.com. See docs/VERSIONS.md.

#[path = "../../../native/tailwind/src/patch.rs"]
#[allow(dead_code)]
mod patch;

use patch::{Entry, Patch};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

type Files = Vec<(String, Vec<u8>)>;

/// The files of a directory, with `/` in their names, sorted.
fn read_files(directory: &Path) -> Result<Files, String> {
    fn walk(root: &Path, directory: &Path, out: &mut Files) -> Result<(), String> {
        let entries = std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else {
                let name = path.strip_prefix(root).unwrap().components().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
                // Not the notes of scripts/packages.sh.
                if name.starts_with('.') {
                    continue;
                }
                let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                out.push((name, bytes));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(directory, directory, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(format!("{} has no files", directory.display()));
    }
    Ok(files)
}

fn write_files(directory: &Path, files: &Files) -> Result<(), String> {
    for (name, bytes) in files {
        let path = directory.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
        std::fs::write(&path, bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

/// The files of a version of a store: its directory, or its patch with
/// the files of the reference version.
fn read_version(store: &Path, version: &str, depth: usize) -> Result<Files, String> {
    if depth > 1024 {
        return Err(format!("the patches of {version} have no end"));
    }
    let directory = store.join(version);
    if directory.is_dir() {
        return read_files(&directory);
    }
    let file = store.join(format!("{version}.patch"));
    let bytes = std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
    let patch = Patch::parse(&bytes).ok_or_else(|| format!("{} is not a patch file", file.display()))?;
    let reference = read_version(store, &patch.reference, depth + 1)?;
    let mut files = Vec::new();
    for (name, entry) in &patch.files {
        let old = || reference.iter().find(|(other, _)| other == name).map(|(_, bytes)| bytes).ok_or_else(|| format!("{} has no {name}", patch.reference));
        let bytes = match entry {
            Entry::Same => old()?.clone(),
            Entry::Delta(delta) => patch::apply(old()?, delta).ok_or_else(|| format!("the delta of {name} in {} does not apply", file.display()))?,
            Entry::Full(bytes) => bytes.clone(),
        };
        files.push((name.clone(), bytes));
    }
    files.sort();
    Ok(files)
}

fn pack(packages: &Path, out: &Path, versions: &[String]) -> Result<(), String> {
    let Some((newest, older)) = versions.split_last() else {
        return Err("pack needs one version or more".to_string());
    };
    std::fs::create_dir_all(out).map_err(|error| error.to_string())?;
    let mut reference_version = newest.clone();
    let mut reference = read_files(&packages.join(newest))?;
    write_files(&out.join(newest), &reference)?;
    let mut total = 0;
    for version in older.iter().rev() {
        let target = read_files(&packages.join(version))?;
        let bytes = Patch::make(&reference_version, &reference, &target).to_bytes();
        total += bytes.len();
        let file = out.join(format!("{version}.patch"));
        std::fs::write(&file, bytes).map_err(|error| format!("{}: {error}", file.display()))?;
        reference_version = version.clone();
        reference = target;
    }
    // Read each version back: the store must give the files of the package.
    for version in versions {
        if read_version(out, version, 0)? != read_files(&packages.join(version))? {
            return Err(format!("the store gives other files for {version}"));
        }
    }
    eprintln!("twpatch: {newest} in full, and {} patches with {total} bytes: each version reads back the same", older.len());
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("pack") if args.len() >= 4 => pack(&PathBuf::from(&args[1]), &PathBuf::from(&args[2]), &args[3..]),
        Some("unpack") if args.len() == 4 => {
            read_version(&PathBuf::from(&args[1]), &args[2], 0).and_then(|files| write_files(&PathBuf::from(&args[3]), &files))
        }
        _ => Err("Usage: twpatch pack PACKAGES OUT VERSION...\n       twpatch unpack STORE VERSION OUT".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("twpatch: {message}");
            ExitCode::FAILURE
        }
    }
}
