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

use std::os::unix::net::UnixStream;
use std::thread::sleep;
use std::time::Duration;

fn _open_socket(path: String, iter: i32) -> Result<UnixStream, std::io::Error> {
    match UnixStream::connect(&path) {
        Ok(stream) => Ok(stream),
        Err(err) => {
            if iter < 10 {
                sleep(Duration::from_secs(1));
                _open_socket(path, iter + 1)
            } else {
                Err(err)
            }
        }
    }
}

pub fn open_socket(path: String) -> Result<UnixStream, std::io::Error> {
    _open_socket(path, 0)
}
