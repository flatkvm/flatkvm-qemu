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

use crate::agent::AgentHost;
use crate::qmpconn::QmpConn;
use serde_derive::{Deserialize, Serialize};
use shlex::split;
use std::process::{Child, Command, Stdio};

#[derive(Debug, Serialize, Deserialize)]
pub enum QemuSharedDirType {
    FlatpakSystemDir,
    FlatpakUserDir,
    FlatpakAppDir,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum QemuSharedDirSecurity {
    Passthrough,
    Mapped,
    None,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QemuSharedDir {
    pub dir_type: QemuSharedDirType,
    pub app_name: String,
    pub source: String,
    pub tag: String,
    pub readonly: bool,
}

pub struct QemuRunner {
    name: String,
    vcpu_num: u32,
    ram_mb: u32,
    template_path: String,
    kernel: String,
    agent_sock_path: Option<String>,
    qmp_sock_path: Option<String>,
    network: bool,
    audio: bool,
    shared_dirs: Vec<QemuSharedDir>,
}

impl QemuRunner {
    pub fn new(name: String) -> QemuRunner {
        QemuRunner {
            name: name,
            vcpu_num: 1,
            ram_mb: 1024,
            template_path: "/usr/share/flatkvm/template.qcow2".to_string(),
            kernel: "/usr/share/flatkvm/vmlinuz.flatkvm".to_string(),
            agent_sock_path: None,
            qmp_sock_path: None,
            network: true,
            audio: true,
            shared_dirs: Vec::new(),
        }
    }

    pub fn vcpu_num(mut self, num: u32) -> Self {
        self.vcpu_num = num;
        self
    }

    pub fn ram_mb(mut self, mb: u32) -> Self {
        self.ram_mb = mb;
        self
    }

    pub fn agent_sock_path(mut self, path: String) -> Self {
        self.agent_sock_path = Some(path);
        self
    }

    pub fn qmp_sock_path(mut self, path: String) -> Self {
        self.qmp_sock_path = Some(path);
        self
    }

    pub fn audio(mut self, audio: bool) -> Self {
        self.audio = audio;
        self
    }

    pub fn network(mut self, network: bool) -> Self {
        self.network = network;
        self
    }

    pub fn shared_dir(
        mut self,
        dir_type: QemuSharedDirType,
        source: String,
        readonly: bool,
    ) -> Self {
        let lastdir = self.shared_dirs.len();
        self.shared_dirs.push(QemuSharedDir {
            dir_type,
            app_name: self.name.to_string(),
            source,
            tag: format!("shareddir{}", lastdir),
            readonly,
        });
        self
    }

    pub fn get_shared_dirs(self) -> Vec<QemuSharedDir> {
        self.shared_dirs
    }

    pub fn run(&self) -> Result<Child, String> {
        let mut cmdline = format!("-nodefaults -name {} -machine pc,accel=kvm,kernel_irqchip -cpu host,pmu=off -smp {} -m {}m -drive if=virtio,file={},snapshot=on -kernel {} -append \"root=/dev/vda quiet\" -device virtio-vga -display gtk",
                              self.name,
                              self.vcpu_num,
                              self.ram_mb,
                              self.template_path,
                              self.kernel);

        if let Some(agent_sock_path) = &self.agent_sock_path {
            cmdline.push_str(&format!(" -device virtio-serial -chardev socket,path={},server,id=flatkvm-agent,nowait -device virtserialport,chardev=flatkvm-agent,name=org.flatkvm.port.0", agent_sock_path));
        }
        if let Some(qmp_sock_path) = &self.qmp_sock_path {
            cmdline.push_str(&format!(" -qmp unix:{},server", qmp_sock_path));
        }
        if self.network {
            cmdline.push_str(" -net nic,model=virtio -net user");
        }
        if self.audio {
            cmdline.push_str(" -soundhw ac97");
        }
        for dir in &self.shared_dirs {
            cmdline.push_str(&format!(
                " -virtfs local,id={},path={},security_model=none,mount_tag={}",
                dir.tag, dir.source, dir.tag
            ));
            if dir.readonly {
                cmdline.push_str(",readonly");
            }
        }
        let args = match split(&cmdline) {
            Some(args) => args,
            None => return Err("can't format arguments".to_string()),
        };

        Command::new("qemu-system-x86_64")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| err.to_string())
    }

    pub fn get_agent(&self) -> Result<AgentHost, String> {
        match &self.agent_sock_path {
            Some(path) => AgentHost::new(path.to_string()),
            None => Err("agent not configured".to_string()),
        }
    }

    pub fn get_qmp_conn(&self) -> Result<QmpConn, String> {
        match &self.qmp_sock_path {
            Some(path) => QmpConn::new(path.to_string()),
            None => Err("agent not configured".to_string()),
        }
    }
}
