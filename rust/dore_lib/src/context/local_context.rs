use std::ffi::OsString;

use async_trait::async_trait;
use std::process::{Command, exit};

use crate::Context;

pub(crate) struct LocalContext;

impl LocalContext {
    fn exec_command(&self, subcommand: &str, args: impl IntoIterator<Item = impl AsRef<str>>) -> ! {        
        let mut full_args = vec![OsString::from("compose"), OsString::from(subcommand)];
        full_args.extend(args.into_iter().map(|s| OsString::from(s.as_ref())));

        let full_command = ["docker"].into_iter().chain(
            full_args.iter().map(|s| s.to_str().unwrap_or("???"))
        ).collect::<Vec<_>>().join(" ");

        #[cfg(windows)]
        {
            use log::{debug, error};

            unsafe {
                // disable Ctrl+C for this app allowing child process to handle it
                // more at https://learn.microsoft.com/en-us/windows/console/setconsolectrlhandler
                windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 1);
            }

            let docker_exit_status = Command::new("docker.exe")
                .args(&full_args)
                .status();

            match docker_exit_status {
                Ok(exit_status) => {                    
                    debug!("{full_command} exit code: {:?}", exit_status.code());
                    exit(exit_status.code().unwrap_or(1))
                },
                Err(e) => {
                    error!("Failed to execute '{full_command}': {e}");
                    exit(1)
                },
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            use log::error;

            let e = Command::new("docker").args(full_args).exec(); // replace current process
            error!("Failed to execute '{full_command}': {e}");
            exit(1);
        }
    }
}

#[async_trait]
impl Context for LocalContext {
    async fn build(&self, services: &[String]) -> anyhow::Result<()> {
        self.exec_command("build", services)
    }

    async fn up(&self, detach: bool, services: &[String]) -> anyhow::Result<()> {
        let mut args = vec![];
        if detach {
            args.push("-d");
        }
        args.extend(services.iter().map(|s| s.as_str()));

        self.exec_command("up", args);
    }

    async fn down(&self, timeout_sec: Option<u32>, drop_volumes: bool, services: &[String]) -> anyhow::Result<()> {
        let mut args = vec![];
        if let Some(timeout) = timeout_sec {
            args.push("-t".to_owned());
            args.push(timeout.to_string());
        }
        if drop_volumes {
            args.push("-v".to_owned());
        }
        args.extend(services.iter().cloned());

        self.exec_command("down", args)
    }
}