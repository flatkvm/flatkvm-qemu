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

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

use serde_derive::{Deserialize, Serialize};
use x11_clipboard::{Clipboard, Source};

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipboardEvent {
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClipboardMessage {
    ClipboardEvent(ClipboardEvent),
}

pub struct ClipboardListener {
    clipboard: Clipboard,
    sender: Sender<ClipboardMessage>,
    used_flag: Arc<AtomicBool>,
}

impl ClipboardListener {
    pub fn new(sender: Sender<ClipboardMessage>, used_flag: Arc<AtomicBool>) -> ClipboardListener {
        let clipboard = Clipboard::new(Source::Clipboard).unwrap();

        ClipboardListener {
            clipboard,
            sender,
            used_flag,
        }
    }

    pub fn spawn_thread(self) {
        thread::spawn(move || loop {
            let val = self
                .clipboard
                .load(
                    self.clipboard.setter.atoms.utf8_string,
                    self.clipboard.setter.atoms.property,
                    true,
                    None,
                )
                .unwrap();

            if self.used_flag.load(Ordering::Relaxed) {
                self.used_flag.store(false, Ordering::Relaxed);
                continue;
            }

            let data = String::from_utf8(val).unwrap();
            let ce = ClipboardEvent { data };
            self.sender
                .send(ClipboardMessage::ClipboardEvent(ce))
                .unwrap();
        });
    }
}
