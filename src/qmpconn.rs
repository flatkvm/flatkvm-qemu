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

use crate::util::open_socket;
use qapi::{qmp, Qmp};
use std::os::unix::net::UnixStream;

pub struct QmpConn {
    stream: UnixStream,
}

impl QmpConn {
    pub fn new(sockpath: String) -> Result<QmpConn, String> {
        let stream = open_socket(sockpath).map_err(|err| err.to_string())?;

        Ok(QmpConn { stream })
    }

    pub fn initialize(&self) -> Result<(), std::io::Error> {
        let mut qmp = Qmp::from_stream(&self.stream);
        qmp.handshake()?;
        Ok(())
    }

    pub fn send_shutdown(&self) -> Result<(), String> {
        let mut qmp = Qmp::from_stream(&self.stream);
        qmp.execute(&qmp::system_powerdown {})
            .unwrap()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}
