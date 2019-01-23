// flatkvm-qemu
// Copyright (C) 2019  Sergio Lopez <slp@sinrega.org>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.

use crate::clipboard::ClipboardEvent;
use crate::dbus_notifications::DbusNotification;
use crate::runner::QemuSharedDir;
use crate::util::open_socket;
use serde_derive::{Deserialize, Serialize};
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::io::BufRead;
use std::io::BufReader;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentReady {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentAck {
    pub status: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentMountRequest {
    pub shared_dir: QemuSharedDir,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRunRequest {
    pub app: String,
    pub user: bool,
    pub dbus_session: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentAppExitCode {
    pub code: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AgentMessage {
    AgentReady(AgentReady),
    AgentAck(AgentAck),
    AgentMountRequest(AgentMountRequest),
    AgentRunRequest(AgentRunRequest),
    AgentAppExitCode(AgentAppExitCode),
    AgentClosed,
    ClipboardEvent(ClipboardEvent),
    DbusNotification(DbusNotification),
}

pub struct AgentHost {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl AgentHost {
    pub fn new(sockpath: String) -> Result<AgentHost, String> {
        let stream = open_socket(sockpath).map_err(|err| err.to_string())?;
        let reader = BufReader::new(stream.try_clone().map_err(|err| err.to_string())?);

        Ok(AgentHost { stream, reader })
    }

    pub fn try_clone(&mut self) -> Result<AgentHost, std::io::Error> {
        let stream = self.stream.try_clone()?;
        let reader = BufReader::new(stream.try_clone()?);

        Ok(AgentHost { stream, reader })
    }

    pub fn read_message(&mut self) -> Result<String, String> {
        let mut line = String::new();

        match self.reader.read_line(&mut line) {
            Ok(_) => Ok(line),
            Err(err) => Err(err.to_string()),
        }
    }

    pub fn wait_handshake(&mut self) -> Result<AgentReady, String> {
        let data = self.read_message()?;

        match serde_json::from_str(&data).map_err(|err| err.to_string())? {
            AgentMessage::AgentReady(ar) => Ok(ar),
            _ => Err("Protocol error".to_string()),
        }
    }

    pub fn send_message(&mut self, msg: &str) -> Result<(), std::io::Error> {
        self.stream.write_all(msg.as_bytes())?;
        self.stream.flush()
    }

    pub fn send_ack(&mut self) -> Result<(), String> {
        let ack = AgentMessage::AgentAck(AgentAck { status: 0 });
        let mut msg = serde_json::to_string(&ack).map_err(|err| err.to_string())?;
        msg.push('\n');
        self.send_message(&msg).map_err(|err| err.to_string())
    }

    pub fn wait_ack(&mut self) -> Result<i32, String> {
        let data = self.read_message()?;

        match serde_json::from_str(&data).map_err(|err| err.to_string())? {
            AgentMessage::AgentAck(msg) => Ok(msg.status),
            _ => Err("Protocol error".to_string()),
        }
    }

    pub fn get_event(&mut self) -> Result<AgentMessage, String> {
        let data = self.read_message()?;

        if data.len() == 0 {
            Ok(AgentMessage::AgentClosed)
        } else {
            match serde_json::from_str(&data).map_err(|err| err.to_string())? {
                AgentMessage::AgentAppExitCode(msg) => Ok(AgentMessage::AgentAppExitCode(msg)),
                AgentMessage::ClipboardEvent(msg) => Ok(AgentMessage::ClipboardEvent(msg)),
                AgentMessage::DbusNotification(msg) => Ok(AgentMessage::DbusNotification(msg)),
                _ => Err("Protocol error".to_string()),
            }
        }
    }

    pub fn send_clipboard_event(&mut self, data: String) -> Result<(), String> {
        let cbe = AgentMessage::ClipboardEvent(ClipboardEvent { data });
        let mut msg = serde_json::to_string(&cbe).map_err(|err| err.to_string())?;
        msg.push('\n');
        self.send_message(&msg).map_err(|err| err.to_string())
    }

    pub fn request_mount(&mut self, shared_dir: QemuSharedDir) -> Result<i32, String> {
        let mr = AgentMessage::AgentMountRequest(AgentMountRequest { shared_dir });
        let mut msg = serde_json::to_string(&mr).map_err(|err| err.to_string())?;
        msg.push('\n');
        self.send_message(&msg).map_err(|err| err.to_string())?;
        self.wait_ack()
    }

    pub fn request_run(
        &mut self,
        app: String,
        user: bool,
        dbus_session: bool,
    ) -> Result<i32, String> {
        let rr = AgentMessage::AgentRunRequest(AgentRunRequest {
            app,
            user,
            dbus_session,
        });
        let mut msg = serde_json::to_string(&rr).map_err(|err| err.to_string())?;
        msg.push('\n');
        self.send_message(&msg).map_err(|err| err.to_string())?;
        self.wait_ack()
    }

    pub fn initialize(&mut self) -> Result<AgentReady, String> {
        match self.wait_handshake() {
            Ok(ar) => {
                self.send_ack()?;
                Ok(ar)
            }
            Err(err) => Err(err),
        }
    }
}

pub struct AgentGuest {
    file: File,
    reader: BufReader<File>,
}

impl AgentGuest {
    pub fn new(vsock_path: std::path::PathBuf) -> Result<AgentGuest, String> {
        let file = OpenOptions::new()
            .write(true)
            .read(true)
            .custom_flags(0)
            .open(&vsock_path)
            .expect("can't open port");

        let reader = BufReader::new(file.try_clone().expect("can't clone file"));

        Ok(AgentGuest { file, reader })
    }

    pub fn try_clone(&mut self) -> Result<AgentGuest, std::io::Error> {
        let file = self.file.try_clone()?;
        let reader = BufReader::new(file.try_clone()?);

        Ok(AgentGuest { file, reader })
    }

    fn read_message(&mut self) -> Result<String, String> {
        let mut line = String::new();

        match self.reader.read_line(&mut line) {
            Ok(_) => Ok(line),
            Err(err) => Err(err.to_string()),
        }
    }

    fn wait_ack(&mut self) -> Result<i32, String> {
        let data = self.read_message()?;

        println!("wait_ack: data={}", data);
        match serde_json::from_str(&data).map_err(|err| err.to_string())? {
            AgentMessage::AgentAck(msg) => Ok(msg.status),
            _ => Err("Protocol error".to_string()),
        }
    }

    pub fn send_message(&mut self, msg: &str) -> Result<(), std::io::Error> {
        self.file.write_all(msg.as_bytes())?;
        self.file.flush()
    }

    pub fn send_ack(&mut self, status: i32) -> Result<(), String> {
        let ack = AgentMessage::AgentAck(AgentAck { status });
        let mut data = serde_json::to_string(&ack).map_err(|err| err.to_string())?;
        data.push('\n');
        self.send_message(&data).map_err(|err| err.to_string())
    }

    pub fn send_exit_code(&mut self, code: i32) -> Result<(), String> {
        let ec = AgentMessage::AgentAppExitCode(AgentAppExitCode { code });
        let mut data = serde_json::to_string(&ec).map_err(|err| err.to_string())?;
        data.push('\n');
        self.send_message(&data).map_err(|err| err.to_string())
    }

    pub fn send_clipboard_event(&mut self, c: ClipboardEvent) -> Result<(), String> {
        let ce = AgentMessage::ClipboardEvent(c);
        let mut data = serde_json::to_string(&ce).map_err(|err| err.to_string())?;
        data.push('\n');
        self.send_message(&data).map_err(|err| err.to_string())
    }

    pub fn send_dbus_notification(&mut self, n: DbusNotification) -> Result<(), String> {
        let dn = AgentMessage::DbusNotification(n);
        let mut data = serde_json::to_string(&dn).map_err(|err| err.to_string())?;
        data.push('\n');
        self.send_message(&data).map_err(|err| err.to_string())
    }

    pub fn do_handshake(&mut self, version: &str) -> Result<i32, String> {
        let msg = AgentMessage::AgentReady(AgentReady {
            version: version.to_string(),
        });
        let mut data = serde_json::to_string(&msg).map_err(|err| err.to_string())?;
        data.push('\n');
        self.send_message(&data).map_err(|err| err.to_string())?;
        self.wait_ack()
    }

    pub fn get_event(&mut self) -> Result<AgentMessage, String> {
        let data = self.read_message()?;
        serde_json::from_str(&data).map_err(|err| err.to_string())
    }
}
