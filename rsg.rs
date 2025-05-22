use std::{
    fs::Metadata,
    num::ParseIntError,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, LazyLock, Mutex},
    time::Duration,
};

#[derive(Debug)]
enum Error {
    MissingCommand,
    MissingTimeout,
    InvalidTimeout(ParseIntError),
    InvalidCommand(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCommand => write!(f, "Missing command to run"),
            Self::MissingTimeout => write!(f, "Option -t requires an argument"),
            Self::InvalidCommand(c) => write!(f, "Invalid command: {c}"),
            Self::InvalidTimeout(e) => write!(f, "Invalid timeout: {e}"),
        }
    }
}

impl From<ParseIntError> for Error {
    fn from(value: ParseIntError) -> Self {
        Self::InvalidTimeout(value)
    }
}

static DEBUG: std::sync::LazyLock<bool> =
    LazyLock::new(|| std::env::vars().any(|(k, _)| k == "RSG_DEBUG"));

fn find_exe(cmd: &str, self_exe: &Option<Metadata>) -> PathBuf {
    let cmd_exe = Path::new(&cmd).to_owned();
    if cmd_exe.parent() != Some(Path::new("")) {
        return cmd_exe;
    }
    let Some(env_path) = std::env::var("PATH").ok() else {
        return cmd_exe;
    };
    for p in env_path.split(':') {
        let candidate = Path::new(p).join(cmd);
        let Some(metadata) = candidate.metadata().ok() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        if let Some(self_exe) = self_exe {
            if metadata.dev() == self_exe.dev() && metadata.ino() == self_exe.ino() {
                if *DEBUG {
                    eprintln!("skipping {} because its ourself", candidate.display());
                }
                continue;
            }
        }
        return candidate;
    }
    cmd_exe
}

fn main() -> Result<(), Error> {
    let mut argv = std::env::args();
    let argv0 = argv.next().unwrap();
    let mut timeout = None;
    let this = Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::InvalidCommand(argv0.clone()))?;
    let cmd = if this == "rsg" {
        // We can accept options
        let opt = argv.next().ok_or(Error::MissingCommand)?;
        if opt == "-t" {
            let opt_arg = argv.next().ok_or(Error::MissingTimeout)?;
            timeout = Some(Ok(opt_arg.parse::<u32>()?));
            argv.next()
        } else {
            Some(opt)
        }
    } else {
        Some(this.to_owned())
    };
    let self_exe = std::fs::metadata("/proc/self/exe").ok();
    if *DEBUG {
        if let Some(e) = &self_exe {
            eprintln!("/proc/self/exe: dev {} ino {}", e.dev(), e.ino());
        }
    }
    let cmd = cmd.ok_or(Error::MissingCommand)?;
    let env_name = format!("RSG_{cmd}_TIMEOUT");
    let timeout = timeout
        .or_else(|| std::env::var(&env_name).ok().map(|v| v.parse()))
        .or_else(|| std::env::var("RSG_TIMEOUT").ok().map(|v| v.parse()))
        .unwrap_or(Ok(5))?;
    println!(
        "Will run {cmd} in {timeout} second{}",
        if timeout != 1 { "s" } else { "" }
    );
    println!("Press <Enter> to confirm, or <Ctrl-C> to cancel");
    let go = Arc::new(Mutex::new(false));
    let cv = Arc::new(Condvar::new());
    let args = argv.collect::<Vec<_>>();
    let countdown = std::thread::spawn({
        let cv = cv.clone();
        let go = go.clone();
        move || {
            use std::io::Write;
            let mut stdout = std::io::stdout();
            for i in (1..=timeout).rev() {
                write!(stdout, "{i} ").ok();
                stdout.flush().ok();
                let go = go.lock().unwrap();
                let (go, _) = cv
                    .wait_timeout_while(go, Duration::from_secs(1), |go| !*go)
                    .unwrap();
                if *go {
                    break;
                }
            }

            println!();

            let cmd_exe = find_exe(&cmd, &self_exe);
            if *DEBUG {
                eprintln!("will use {}", cmd_exe.display())
            }

            let status = match std::process::Command::new(&cmd_exe).args(args).status() {
                Err(e) => {
                    eprintln!("\nFailed to start {cmd}: {e}");
                    1
                }
                Ok(status) => {
                    println!("\n{cmd} finished with {status}");
                    status.code().unwrap_or(0)
                }
            };

            std::process::exit(status);
        }
    });
    let mut line = String::new();
    let _: Result<_, _> = std::io::stdin().read_line(&mut line);
    *go.lock().unwrap() = true;
    cv.notify_one();

    // Waiting for the countdown thread to terminate the process.
    countdown.join().unwrap();
    Ok(())
}
