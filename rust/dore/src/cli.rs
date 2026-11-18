use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) commands: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Execute file sync and call `docker compose up [SERVICE...]` on remote host
    Up {
        /// Detached mode: Run containers in the background
        #[arg(short, long)]
        detach: bool,

        services: Vec<String>
    },

    /// Call `docker compose down` on remote host
    Down {
        /// Specify a shutdown timeout in seconds
        #[arg(short, long)]
        timeout: Option<u32>,

        /// Remove named volumes declared in the "volumes" section of the Compose file and anonymous volumes attached to containers
        #[arg(short = 'v', long = "volumes")]
        drop_volumes: bool,

        services: Vec<String>
    },

    /// Execute file sync and call `docker compose build` on remote host
    Build {
        services: Vec<String>
    }
}