use std::sync::Arc;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::stream::StreamExt;
use log::{warn, info};
use russh::ChannelMsg;
use tokio::io::AsyncWriteExt;

use super::SshSession;

pub(crate) async fn run_interactive_command(session: Arc<SshSession>, command: &str) -> Result<u32> {
    let mut channel = session.create_channel().await?;

    let (cols, rows) = crossterm::terminal::size()?;
    channel.request_pty(
        false,
        "xterm-256colors", 
        cols as u32, 
        rows as u32, 
        0, 
        0, 
        &[]
    ).await?;

    info!("Executing command '{command}'");
    channel.exec(true, command).await?;
    let _raw_mode = RawModeSwitcher::new()?;

    let mut event_stream = EventStream::new();
    let mut stdout = tokio::io::stdout();
    let mut exit_code = 0;

    loop {
        tokio::select! {
            Some(Ok(event)) = event_stream.next() => {
                match event {
                    Event::Resize(w, h) => {
                        channel.window_change(w as u32, h as u32, 0, 0).await?;
                    },
                    Event::Key(key_event) => {
                        if let Some(data) = key_to_bytes(key_event) {
                            channel.data(&data[..]).await?;
                        }
                    },
                    Event::Paste(text) => {
                        channel.data(text.as_bytes()).await?;
                    },
                    _ => {}
                }
            },
            Some(msg) = channel.wait() => {
                match msg {
                    ChannelMsg::Data { ref data } | ChannelMsg::ExtendedData { ref data, .. } => {
                        stdout.write_all(data).await?;
                        stdout.flush().await?;
                    },
                    ChannelMsg::ExitStatus { exit_status } => {
                        exit_code = exit_status;
                        if exit_code != 0 {
                            warn!("Remote command '{command}' exited with code {exit_code}");
                        }
                    },                    
                    ChannelMsg::Eof => {
                        channel.eof().await?;
                    },
                    ChannelMsg::Close => {
                        break;
                    },
                    _ => {}
                }
            }
        }
    }

    let _ = channel.close().await;

    stdout.write_all(b"\n").await?;
    stdout.flush().await?;

    Ok(exit_code)
}


fn key_to_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    if key.is_release() {
        return None;
    }

    let with_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(c) => {
            if with_ctrl && c.is_ascii() {
                let c = c.to_ascii_lowercase();
                Some(vec![(c as u8) - b'a' + 1])
            }
            else {
                let mut buf = vec![0u8; 4];
                c.encode_utf8(&mut buf);
                Some(buf)
            }
        },
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![127]),
        KeyCode::Tab => Some(vec![b'\t']),
        _ => None
    }
}
struct RawModeSwitcher;

impl RawModeSwitcher {
    pub fn new() -> Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeSwitcher {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().ok();
    }
}

