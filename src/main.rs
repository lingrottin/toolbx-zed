use std::cell::RefCell;
use std::env::consts::EXE_EXTENSION;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::os::fd::OwnedFd;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use log::LevelFilter;
use simplelog::{CombinedLogger, Config, WriteLogger};
static WAIT_FOR_SIGINT_ONCELOCK: OnceLock<bool> = OnceLock::new();
static LOG_INIT_ONCELOCK: OnceLock<()> = OnceLock::new();

// a very simple anyhow::Error imitation
struct BnyhowError {
    message: String,
}
impl<E> From<E> for BnyhowError
where
    E: std::error::Error,
{
    fn from(error: E) -> Self {
        BnyhowError {
            message: error.to_string(),
        }
    }
}
type Result<T> = std::result::Result<T, BnyhowError>;

macro_rules! bnyhow {
    ($($arg:tt)*) => {
        BnyhowError {
            message: format!($($arg)*),
        }
    };
}

fn init_logging() {
    let debug_enabled = std::env::var("TOOLBX_ZED_DEBUG").is_ok();
    let debug_build = cfg!(debug_assertions);

    if !debug_enabled && !debug_build {
        return;
    }

    LOG_INIT_ONCELOCK.get_or_init(|| {
        let log_dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let user = user_id().unwrap_or_else(|_| "0".to_string());
                PathBuf::from(format!("/run/user/{}", user))
            });
        let log_path = log_dir.join("toolbx-zed.log");

        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file = match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!(
                    "Failed to open debug log file {}: {}",
                    log_path.display(),
                    e
                );
                return;
            }
        };

        let _ = CombinedLogger::init(vec![WriteLogger::new(
            LevelFilter::Trace,
            Config::default(),
            file,
        )]);
    });
}

fn main() {
    init_logging();
    if let Err(e) = init_wrapper_bins() {
        log::error!("Failed to create wrapper binaries: {}", e.message);
    }
    log::info!("toolbx-zed starting");
    let mut args = std::env::args().collect::<Vec<String>>();
    let executable = args.remove(0);
    let executable_name = executable
        .split(std::path::MAIN_SEPARATOR)
        .last()
        .unwrap_or("")
        .trim_end_matches(EXE_EXTENSION);
    match executable_name {
        "ssh" => ssh(args),
        "sftp" => sftp(args),
        _ => zed(args),
    }
    .unwrap_or_else(|e| {
        log::error!("Error: {}", e.message);
        std::process::exit(1);
    });
}

fn is_toolbx() -> Result<bool> {
    Ok(std::fs::exists("/run/.toolboxenv")? && std::fs::exists("/run/.containerenv")?)
}

fn xdg_data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_else(|| "/".into());
            PathBuf::from(home).join(".local/share")
        })
}

fn init_wrapper_bins() -> Result<()> {
    let bin_dir = xdg_data_home().join("toolbx-zed-bin");
    std::fs::create_dir_all(&bin_dir)?;
    let current_exe = std::env::current_exe()?;
    let ssh_link = bin_dir.join("ssh");
    let sftp_link = bin_dir.join("sftp");

    let _ = std::fs::remove_file(&ssh_link);
    let _ = std::fs::remove_file(&sftp_link);

    symlink(&current_exe, &ssh_link)?;
    symlink(&current_exe, &sftp_link)?;

    Ok(())
}

struct ContainerEnv {
    id: String,
}

impl ContainerEnv {
    fn load() -> Result<Self> {
        let content = std::fs::read_to_string("/run/.containerenv")?;
        let mut id = None;
        for line in content.lines() {
            if line.starts_with("id=") {
                id = Some(line[3..].trim_matches('"').to_string());
            }
        }
        Ok(Self {
            id: id.ok_or_else(|| bnyhow!("ID not found in /run/.containerenv"))?,
        })
    }
}

fn user_id() -> Result<String> {
    let output = Command::new("id").arg("-u").output()?;
    if !output.status.success() {
        return Err(bnyhow!("Failed to execute id command"));
    }
    let stdout = String::from_utf8(output.stdout)?;
    let user_id = stdout.trim().to_string();
    Ok(user_id)
}

// Get the PATH environment variable and remove the current executable's directory from it
// This is necessary because this tool itself may be named as "zed" but we want to call the
// actual Zed editor, not this wrapper
fn get_path() -> OsString {
    let path = std::env::var_os("PATH");
    if path.is_none() {
        return "".into();
    }
    let path = path.unwrap();
    let splitted = std::env::split_paths(&path).collect::<Vec<_>>();
    let current_exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::new());
    let filtered = splitted
        .into_iter()
        .filter(|p| p != &current_exe.parent().unwrap_or_else(|| "".as_ref()))
        .collect::<Vec<_>>();
    std::env::join_paths(filtered).unwrap_or_else(|_| "".into())
}

fn check_binary_exists(cmd: &str) -> bool {
    log::debug!("checking whether binary exists: {}", cmd);
    let path = get_path();
    match Command::new(cmd).env("PATH", path).spawn() {
        Ok(mut child) => {
            let _ = child.kill(); // Kill it immediately if it started
            true
        }
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

fn get_zed(extra_path: Option<String>) -> Result<RefCell<Command>> {
    log::debug!("resolving zed executable");
    // prefer flatpak-spawn because we want to work in toolbx
    // See https://containertoolbx.org/doc
    // > Images SHOULD have the flatpak-spawn(1) command. Otherwise, it won’t be possible
    // > to use toolbox(1) inside containers created from those images.
    if check_binary_exists("flatpak-spawn") {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg("flatpak").arg("run");
        if let Some(p) = extra_path {
            cmd.arg("--command=env");
            cmd.arg("dev.zed.Zed");
            cmd.arg(format!("PATH={}:/app/bin:/usr/bin:/bin", p));
            cmd.arg("/app/bin/zed-wrapper");
        } else {
            cmd.arg("dev.zed.Zed");
        }
        Ok(RefCell::new(cmd))
    } else if check_binary_exists("flatpak") {
        let mut cmd = Command::new("flatpak");
        cmd.arg("run");
        if let Some(p) = extra_path {
            cmd.arg("--command=env");
            cmd.arg("dev.zed.Zed");
            cmd.arg(format!("PATH={}:/app/bin:/usr/bin:/bin", p));
            cmd.arg("/app/bin/zed-wrapper");
        } else {
            cmd.arg("dev.zed.Zed");
        }
        Ok(RefCell::new(cmd))
    } else if check_binary_exists("zed") {
        let mut cmd = Command::new("zed");
        if let Some(p) = extra_path {
            cmd.env("PATH", format!("{}:{}", p, get_path().to_string_lossy()));
        }
        Ok(RefCell::new(cmd))
    } else if check_binary_exists("zeditor") {
        let mut cmd = Command::new("zeditor");
        if let Some(p) = extra_path {
            cmd.env("PATH", format!("{}:{}", p, get_path().to_string_lossy()));
        }
        Ok(RefCell::new(cmd))
    } else {
        Err(bnyhow!("Zed editor not found in PATH"))
    }
}

fn zed(args: Vec<String>) -> Result<()> {
    log::info!("running zed wrapper with {} args", args.len());
    let is_toolbx = is_toolbx()?;
    let container_env = if is_toolbx {
        Some(ContainerEnv::load()?)
    } else {
        log::info!("Not in a container, running as-is");
        None
    };
    let process_zed_arg = |arg: &str| {
        if !is_toolbx {
            return arg.to_string();
        }

        if arg.starts_with("-")
            || arg.starts_with("/")
            || (arg.contains("://") && !arg.starts_with("file://"))
        {
            // flags, absolute paths and URLs are not modified
            return arg.to_string();
        }

        let arg = if arg.starts_with("file://") {
            arg.trim_start_matches("file://").to_string()
        } else if arg.starts_with("~") {
            arg.replace(
                "~",
                &std::env::var("HOME").unwrap_or_else(|_| "~".to_string()),
            )
        } else {
            arg.to_string()
        };

        let absolute = std::path::absolute(arg.clone()).unwrap_or(arg.into());
        format!(
            "ssh://{}@{}.toolbx/{}",
            user_id().unwrap_or("user".to_string()),
            container_env.as_ref().unwrap().id,
            absolute.display().to_string().trim_start_matches("/")
        )
    };
    // parse arguments
    log::debug!("zed raw args: {:?}", args);
    let mut actual_args = Vec::new();
    let mut asis = container_env.is_none();
    let mut cur_param = 0;
    let mut escaped = false;
    let mut iteration = args.clone().into_iter();
    if !asis {
        'parse: while let Some(arg) = iteration.next() {
            if arg == "--help" {
                println!("Usage: toolbx-zed [OPTIONS] [PATHS]...");
                println!(
                    "A wrapper for the Zed editor that configures it to run on a Toolbx container"
                );
                println!();
                println!("Options:");
                println!("  --help    Print help information");
                if let Ok(cmd) = get_zed(None) {
                    println!();
                    cmd.borrow_mut().arg("--help").spawn()?.wait()?;
                } else {
                    println!();
                    println!(
                        "WARNING: Zed editor or flatpak-spawn or flatpak not found in PATH, this wrapper will not work"
                    );
                }
                return Ok(());
            } else {
                match arg.as_str() {
                    "--zed" => cur_param = 1,
                    "--user-data-dir" => cur_param = 1,
                    "--dev-server-token" => {
                        asis = true;
                        break 'parse;
                    }
                    "--" => escaped = true,
                    _ => {}
                }
                if !arg.starts_with("-") && cur_param > 0 {
                    // the spicified parameters should not be altered
                    cur_param -= 1;
                    actual_args.push(arg);
                    continue;
                }
                if !escaped && arg.starts_with("-") {
                    actual_args.push(arg);
                    continue;
                }
                actual_args.push(process_zed_arg(&arg));
            }
        }
    }
    let args = if asis {
        log::debug!("zed arguments passed through as-is");
        args.into_iter().collect()
    } else {
        actual_args
    };

    // Run Zed with the processed arguments
    log::debug!("preparing base PATH for zed");
    let mut path = None;
    if let Some(container_env) = container_env.as_ref() {
        let wrapper_dir = xdg_data_home().join("toolbx-zed-bin");
        log::debug!(
            "using wrapper binaries from {} for container {}",
            wrapper_dir.display(),
            container_env.id
        );
        path = Some(wrapper_dir.display().to_string());
    }

    let cmd = get_zed(path)?;
    let mut cmd = cmd.borrow_mut();
    for arg in args {
        cmd.arg(arg);
    }
    log::debug!("Launching Zed with processed arguments: {:?}", cmd);
    cmd.stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    let status = cmd.spawn()?.wait()?;
    if !status.success() {
        log::error!("Zed process exited with status: {}", status);
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn parse_ssh_dest(dest: &str) -> Option<(String, String)> {
    log::debug!("parsing ssh destination: {}", dest);
    // parse the custom ssh destination that we created above, and extract the container id
    // the format is user@container_id.toolbx
    if let Some(at_pos) = dest.find('@') {
        let before_at = &dest[..at_pos];
        let after_at = &dest[at_pos + 1..];
        if let Some(dot_pos) = after_at.find(".toolbx") {
            return Some((before_at.to_string(), after_at[..dot_pos].to_string()));
        }
    }
    None
}

fn get_podman() -> RefCell<Command> {
    log::debug!("resolving podman command");
    // This is usually ran by Zed in its flatpak sandbox
    // check Podman binary first (for editor like Zed, it is possible
    // for users to expose the host PATH to the sandbox, so we check
    // if podman is available first) And if podman is not available,
    // we use flatpak-spawn to call Podman on the host.

    if check_binary_exists("podman") {
        RefCell::new(Command::new("podman"))
    } else {
        log::info!(
            "podman binary not found in PATH, using flatpak-spawn to call podman on the host"
        );
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg("podman");
        RefCell::new(cmd)
    }
}

fn ssh(args: Vec<String>) -> Result<()> {
    log::info!("running ssh wrapper with {} args", args.len());
    // only include ones that Zed uses here
    let flags_with_parameter = ["-o", "-L", "-p"];
    // parse args

    let mut pseudo_term = true;

    let mut master = false;
    let mut param = None;
    let mut idx = 0;
    let mut dest = None;
    log::debug!("ssh raw args: {:?}", args);
    let mut iter = args.clone().into_iter();
    while let Some(arg) = iter.next() {
        idx += 1;
        if arg == "--help" {
            println!("Usage: toolbx-zed(ssh) [OPTIONS] [SSH_ARGS]...");
            println!("A wrapper for ssh that intercepts Zed's remote development connections");
            println!("and routes them to a Toolbx container via podman.");
            println!();
            println!("Options:");
            println!("  --help    Print help information");
            return Ok(());
        }

        if flags_with_parameter.contains(&arg.as_str()) {
            param = Some(arg)
        } else if !arg.starts_with("-") && param.is_some() {
            if arg.as_str() == "ControlMaster=yes" {
                master = true;
            }
            param = None;
            // currently these args are ignored
        } else if !arg.starts_with("-") {
            dest = Some(arg);
            break;
        }
        // process flags that we care about
        else if arg.as_str() == "-T" {
            pseudo_term = false;
        } else if arg.as_str() == "-t" {
            pseudo_term = true;
        } else if arg.as_str() == "-N" {
            master = true;
        }
    }
    let mut cmd_args: Vec<String>;
    if dest.is_none() || (args.len() == idx && !master) {
        log::debug!("no destination specified for ssh command");
        return Err(bnyhow!("ERROR: No destination specified for ssh command"));
    } else if master {
        // sleep until termination signal or ctrl-c

        // Zed use ControlMaster=yes to keep the ssh connection alive, but don't use that process's
        // stdio for communication. Instead it looks for EOF in stdout of that process to determine
        // when the connection is established, and then keep the process alive until the user
        // terminate the connection. (this behavior is indicated by ControlMaster=yes and -N) So here
        // we close the stdout immediately and then wait a termination signal.

        // Closing stdout using `std` functions requires a nightly toolchain as of now, so we use
        // libc directly. Code below basically comes from the nightly api `StdioExt::take_fd()`
        //
        // See https://doc.rust-lang.org/std/os/unix/io/trait.StdioExt.html for more details.
        {
            use std::os::fd::AsRawFd;
            let stdout = std::io::stdout();
            log::debug!("dropping stdout");
            fn null_fd() -> std::io::Result<OwnedFd> {
                let null_dev = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open("/dev/null")?;
                Ok(null_dev.into())
            }
            let null_fd = null_fd()?;
            unsafe {
                libc::dup2(null_fd.as_raw_fd(), stdout.as_raw_fd());
            }
            log::debug!("stdout dropped");
        }

        let _ = ctrlc::set_handler(|| {
            WAIT_FOR_SIGINT_ONCELOCK.get_or_init(|| true);
        });
        let _ = WAIT_FOR_SIGINT_ONCELOCK.wait();

        // Cleanup
        let path = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::new())
            .parent()
            .unwrap_or_else(|| "".as_ref())
            .join("toolbx-zed-tmp-bin");
        let _ = std::fs::remove_file(path.join("ssh"));
        let _ = std::fs::remove_file(path.join("sftp"));
        let _ = std::fs::remove_dir(path);

        // matching OpenSSH's behavior
        // 255 indicates a common ssh connection error, and this includes
        // our case where ssh is killed by SIGINT after connection is established,
        // and -N is specified.
        std::process::exit(255);
    } else {
        cmd_args = args.clone().into_iter().skip(idx).collect();
    }

    let dest = parse_ssh_dest(&dest.unwrap());
    if dest.is_none() {
        log::debug!("destination is not toolbx-formatted; delegating to system ssh/sftp");
        // Destination is not in the expected format, just run ssh as is
        let mut cmd = std::process::Command::new("ssh");
        cmd.env("PATH", get_path());
        for arg in args {
            cmd.arg(arg);
        }
        cmd.stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
        let status = cmd.status()?;
        if !status.success() {
            return Err(BnyhowError {
                message: format!("ssh command failed with status: {}", status),
            });
        }
        return Ok(());
    }

    // somehow Zed puts -T flag after destination, so we perform a double-check here
    if cmd_args[0] == "-T" {
        pseudo_term = false;
        cmd_args.remove(0);
    }

    let (user_id, container_id) = dest.unwrap();
    log::debug!("ssh destination resolved to container id: {}", container_id);
    let cmd = get_podman();
    let mut cmd = cmd.borrow_mut();
    cmd.arg("exec").arg("--user").arg(user_id).arg("-i");

    if pseudo_term {
        cmd.arg("-t");
    }

    cmd.arg(container_id).arg("sh").arg("-c");

    for arg in cmd_args {
        cmd.arg(arg);
    }
    cmd.stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    let status = cmd.spawn()?.wait()?;
    if !status.success() {
        log::error!("command failed with status: {}", status);
        // SSH returns the return code of the remote command, we want to do the same
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn sftp(args: Vec<String>) -> Result<()> {
    log::info!("running sftp wrapper with {} args", args.len());
    // only include ones that Zed uses here
    // "-b" is followed by "-" in these cases, which will be ignored by .starts_with("-"), so we don't
    // count it here
    let flags_with_parameter = ["-o", "-P"];
    let mut dest = None;
    let mut param = false;
    log::debug!("sftp raw args: {:?}", args);
    for arg in args.clone() {
        if arg == "--help" {
            println!("Usage: toolbx-zed(sftp) [OPTIONS] [SFTP_ARGS]...");
            println!("A wrapper for sftp that intercepts Zed's remote development file transfers");
            println!("and routes them to a Toolbx container via podman.");
            println!();
            println!("Options:");
            println!("  --help    Print help information");
            return Ok(());
        }
        if flags_with_parameter.contains(&arg.as_str()) {
            param = true;
        } else if !arg.starts_with("-") && param {
            // parameter of a flag, ignore
            param = false;
        } else if !arg.starts_with("-") && dest.is_none() {
            dest = Some(arg);
            break;
        }
    }

    if dest.is_none() {
        return Err(bnyhow!("ERROR: No destination specified for sftp command"));
    }

    let dest = parse_ssh_dest(&dest.unwrap());
    if dest.is_none() {
        // Destination is not in the expected format, just run sftp as is
        let mut cmd = std::process::Command::new("sftp");
        cmd.env("PATH", get_path());
        for arg in args {
            cmd.arg(arg);
        }
        cmd.stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
        let status = cmd.status()?;
        if !status.success() {
            return Err(BnyhowError {
                message: format!("sftp command failed with status: {}", status),
            });
        }
        return Ok(());
    }

    let (user_id, container_id) = dest.unwrap();

    log::debug!(
        "sftp destination resolved to container id: {}",
        container_id
    );
    // parse batch input
    // Zed only inputs 1 command each launch, so we don't need to handle multiple commands here
    // format: "put [-r] local_path remote_path"

    // Recurse is the default behavior of podman-copy (and there's no way to turn it off) so we
    // ignore this
    //
    // let mut recursive = false;

    let local_path;
    let remote_path;
    let mut buffer = String::new();
    log::debug!("waiting for sftp batch input");
    std::io::stdin().read_line(&mut buffer)?;

    let parts = buffer.trim().split_whitespace().collect::<Vec<_>>();
    if parts.len() < 3 {
        return Err(bnyhow!("Invalid batch command format"));
    }
    if parts[0] == "put" {
        if parts[1] == "-r" {
            // recursive = true;
            local_path = Some(parts[2].trim_matches('"'));
            remote_path = Some(parts[3].trim_matches('"'));
        } else {
            local_path = Some(parts[1].trim_matches('"'));
            remote_path = Some(parts[2].trim_matches('"'));
        }
    } else {
        return Err(bnyhow!("Unsupported batch command: {}", parts[0]));
    }

    if local_path.is_none() || remote_path.is_none() {
        return Err(bnyhow!("Local path or remote path is missing"));
    }

    log::debug!(
        "executing podman cp for sftp upload: podman cp {} {}:{}",
        local_path.unwrap(),
        container_id,
        remote_path.unwrap()
    );
    let cmd = get_podman();
    let mut cmd = cmd.borrow_mut();
    cmd.arg("cp").arg(local_path.unwrap()).arg(format!(
        "{}:{}",
        container_id,
        remote_path.unwrap()
    ));
    let status = cmd.spawn()?.wait()?;
    if !status.success() {
        return Err(bnyhow!("command failed with status: {}", status));
    }

    // change ownership of the uploaded file to the user to avoid permission issues
    //
    // By default the default user of Toolbx images is a pseudo root user in the container
    // but we usually work as the host user (users on the host and in the container share the
    // same HOME and UID and GID), so the uploaded file will be owned by pseudo root and not
    // writable by the user. We need to change the ownership of the uploaded file to the user.
    let cmd = get_podman();
    let mut cmd = cmd.borrow_mut();
    cmd.arg("exec")
        .arg(container_id)
        .arg("sh")
        .arg("-c")
        .arg(format!(
            "chown -R {}:{} {}",
            user_id,
            user_id,
            remote_path.unwrap()
        ))
        .spawn()?
        .wait()?;

    Ok(())
}
