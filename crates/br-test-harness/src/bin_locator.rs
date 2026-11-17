use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const OUTPUT_ROOT_MARKER: &str = "CACHEDIR.TAG";
const PARENTS_SEARCHED: usize = 4;

pub fn workspace_bin(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    match locate(name, &exe, cargo_dirs) {
        Ok(bin) => bin,
        Err(miss) => panic!("{miss}"),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CargoDirs {
    target: PathBuf,
    build: PathBuf,
}

#[derive(Debug)]
struct Miss {
    name: String,
    exe: PathBuf,
    looked: Vec<PathBuf>,
    cargo: String,
}

impl fmt::Display for Miss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Miss {
            name,
            exe,
            looked,
            cargo,
        } = self;
        writeln!(f, "{name} must be built by the workspace; no file at:")?;
        for path in looked {
            writeln!(f, "  {}", path.display())?;
        }
        writeln!(f, "test executable: {}", exe.display())?;
        writeln!(f, "{cargo}")?;
        write!(
            f,
            "build it (cargo build -p <its package> --bin {name}); a target-dir or build-dir set only on the cargo command line is invisible to the test, so name it in .cargo/config.toml or the environment"
        )
    }
}

fn locate(
    name: &str,
    exe: &Path,
    cargo_dirs: impl FnOnce() -> Result<CargoDirs, String>,
) -> Result<PathBuf, Miss> {
    let mut looked = beside(exe, name, |dir| dir.join(OUTPUT_ROOT_MARKER).is_file());
    if let Some(bin) = first_file(&looked) {
        return Ok(bin);
    }

    let cargo = match cargo_dirs() {
        Ok(dirs) => {
            let across = rerooted(&canonical(exe), name, &dirs.target, &canonical(&dirs.build));
            if let Some(bin) = first_file(&across) {
                return Ok(bin);
            }
            for path in across {
                if !looked.contains(&path) {
                    looked.push(path);
                }
            }
            format!(
                "cargo metadata: target-dir {}, build-dir {}",
                dirs.target.display(),
                dirs.build.display()
            )
        }
        Err(why) => format!("cargo metadata: {why}"),
    };

    Err(Miss {
        name: name.to_owned(),
        exe: exe.to_path_buf(),
        looked,
        cargo,
    })
}

fn beside(exe: &Path, name: &str, is_output_root: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    exe.ancestors()
        .skip(1)
        .take(PARENTS_SEARCHED + 1)
        .take_while(|dir| !is_output_root(dir))
        .map(|dir| dir.join(name))
        .collect()
}

fn rerooted(exe: &Path, name: &str, target: &Path, build: &Path) -> Vec<PathBuf> {
    let Some(inside) = exe.parent().and_then(|dir| dir.strip_prefix(build).ok()) else {
        return Vec::new();
    };
    inside
        .ancestors()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| target.join(dir).join(name))
        .collect()
}

fn first_file(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.is_file()).cloned()
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn cargo_dirs() -> Result<CargoDirs, String> {
    static DIRS: OnceLock<Result<CargoDirs, String>> = OnceLock::new();
    DIRS.get_or_init(ask_cargo).clone()
}

fn ask_cargo() -> Result<CargoDirs, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(&cargo);
    command.args(["metadata", "--format-version", "1", "--no-deps"]);
    if let Some(dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        command.current_dir(dir);
    }
    let output = command
        .output()
        .map_err(|e| format!("could not run {}: {e}", cargo.to_string_lossy()))?;
    if !output.status.success() {
        return Err(format!(
            "{} metadata failed ({}): {}",
            cargo.to_string_lossy(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_cargo_dirs(&output.stdout)
}

fn parse_cargo_dirs(metadata: &[u8]) -> Result<CargoDirs, String> {
    let metadata: serde_json::Value =
        serde_json::from_slice(metadata).map_err(|e| format!("output is not JSON: {e}"))?;
    let target = metadata["target_directory"]
        .as_str()
        .map(PathBuf::from)
        .ok_or("output has no target_directory")?;
    let build = metadata["build_directory"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| target.clone());
    Ok(CargoDirs { target, build })
}

#[cfg(test)]
mod tests;
