// This file is a part of simple_audio
// Copyright (c) 2022-2023 Erikas Taroza <erikastaroza@gmail.com>

use std::{
    io::{Read, Seek},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
};

use anyhow::{anyhow, Context};
use rangemap::RangeSet;
use reqwest::{blocking::Client, StatusCode};
use symphonia::core::io::MediaSource;

use super::{
    streamable::{Streamable, CHUNK_SIZE},
    AnniSource,
};

pub struct HttpStream {
    url: String,
    client: Client,
    buffer: Vec<u8>,
    read_position: usize,
    downloaded: RangeSet<usize>,
    buffer_signal: Arc<AtomicBool>,
}

impl HttpStream {
    pub fn new(url: String, buffer_signal: Arc<AtomicBool>) -> anyhow::Result<Self> {
        Self::with_client(url, Client::new(), buffer_signal)
    }

    pub fn with_client(
        url: String,
        client: Client,
        buffer_signal: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        let content_length = client
            .head(&url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .ok()
            .and_then(|response| response.content_length());
        let buffer = if let Some(content_length) = content_length {
            let length = usize::try_from(content_length).context("audio source is too large")?;
            let mut buffer = Vec::new();
            buffer
                .try_reserve_exact(length)
                .context("could not allocate the HTTP audio buffer")?;
            buffer.resize(length, 0);
            buffer
        } else {
            let response = client.get(&url).send()?.error_for_status()?;
            response.bytes()?.to_vec()
        };
        let mut downloaded = RangeSet::new();
        if content_length.is_none() {
            downloaded.insert(0..buffer.len());
        }
        buffer_signal.store(false, Ordering::Release);

        Ok(Self {
            url,
            client,
            buffer,
            read_position: 0,
            downloaded,
            buffer_signal,
        })
    }

    fn fetch_chunk(&mut self, start: usize) -> anyhow::Result<()> {
        if self.buffer.is_empty() || start >= self.buffer.len() {
            return Ok(());
        }
        let end_exclusive = (start + CHUNK_SIZE).min(self.buffer.len());
        self.buffer_signal.store(true, Ordering::Release);
        let result = (|| {
            let response = self
                .client
                .get(&self.url)
                .header("Range", format!("bytes={start}-{}", end_exclusive - 1))
                .send()?
                .error_for_status()?;
            let status = response.status();
            let bytes = response.bytes()?;

            if status == StatusCode::OK {
                if bytes.is_empty() {
                    return Err(anyhow!("full response returned an empty body"));
                }

                // A 200 response to a range request is the complete
                // representation. Keep it instead of discarding all but one
                // chunk and downloading the same file again on the next read.
                self.buffer = bytes.to_vec();
                self.downloaded = RangeSet::new();
                self.downloaded.insert(0..self.buffer.len());
                return Ok(());
            }

            if status != StatusCode::PARTIAL_CONTENT {
                return Err(anyhow!("server did not honor byte range request"));
            }

            let count = bytes.len().min(end_exclusive - start);
            if count == 0 {
                return Err(anyhow!("range request returned an empty body"));
            }
            self.buffer[start..start + count].copy_from_slice(&bytes[..count]);
            self.downloaded.insert(start..start + count);
            Ok(())
        })();
        self.buffer_signal.store(false, Ordering::Release);
        result
    }
}

impl Streamable for HttpStream {
    fn read_chunk(
        sender: Sender<(usize, Vec<u8>)>,
        url: String,
        start: usize,
        file_size: usize,
    ) -> anyhow::Result<()> {
        if file_size == 0 || start >= file_size {
            sender.send((start, Vec::new())).ok();
            return Ok(());
        }
        let end_exclusive = (start + CHUNK_SIZE).min(file_size);
        let response = Client::new()
            .get(url)
            .header("Range", format!("bytes={start}-{}", end_exclusive - 1))
            .send()?
            .error_for_status()?;
        let status = response.status();
        let bytes = response.bytes()?;
        let (position, chunk) = if status == StatusCode::PARTIAL_CONTENT {
            (
                start,
                bytes[..bytes.len().min(end_exclusive - start)].to_vec(),
            )
        } else if status == StatusCode::OK {
            (0, bytes.to_vec())
        } else {
            return Err(anyhow!("server did not honor byte range request"));
        };
        if chunk.is_empty() {
            return Err(anyhow!("range request returned an empty body"));
        }
        sender.send((position, chunk)).ok();
        Ok(())
    }

    fn try_write_chunk(&mut self, _should_buffer: bool) {
        let (should_fetch, start) = self.should_get_chunk();
        if should_fetch && let Err(error) = self.fetch_chunk(start) {
            // The legacy Streamable contract cannot return an error. Keep the
            // range missing so this method or Read::read can retry it later.
            log::error!("failed to fetch HTTP stream chunk: {error}");
        }
    }

    fn should_get_chunk(&self) -> (bool, usize) {
        match self.downloaded.get(&self.read_position) {
            Some(range) => (range.end < self.buffer.len(), range.end),
            None => (true, self.read_position),
        }
    }
}

impl Read for HttpStream {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() || self.read_position >= self.buffer.len() {
            return Ok(0);
        }

        if !self.downloaded.contains(&self.read_position) {
            self.fetch_chunk(self.read_position)
                .map_err(std::io::Error::other)?;
        }
        let Some(downloaded) = self.downloaded.get(&self.read_position) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "range request returned no readable bytes",
            ));
        };
        let end = (self.read_position + output.len())
            .min(downloaded.end)
            .min(self.buffer.len());
        let bytes = &self.buffer[self.read_position..end];
        output[..bytes.len()].copy_from_slice(bytes);
        self.read_position = end;
        Ok(bytes.len())
    }
}

impl Seek for HttpStream {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        let position = match position {
            std::io::SeekFrom::Start(position) => i128::from(position),
            std::io::SeekFrom::Current(offset) => self.read_position as i128 + i128::from(offset),
            std::io::SeekFrom::End(offset) => self.buffer.len() as i128 + i128::from(offset),
        };
        let position: usize = position.try_into().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid seek position")
        })?;
        if position > self.buffer.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek position is past end of source",
            ));
        }
        self.read_position = position;
        Ok(position as u64)
    }
}

impl MediaSource for HttpStream {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.buffer.len() as u64)
    }
}

impl AnniSource for HttpStream {}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{atomic::AtomicBool, Arc},
        thread,
    };

    use reqwest::blocking::Client;

    use super::{HttpStream, Streamable, CHUNK_SIZE};

    #[test]
    fn a_server_ignoring_ranges_is_downloaded_only_once() {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("skipping loopback HTTP test: {error}");
                return;
            }
            Err(error) => panic!("could not bind loopback test server: {error}"),
        };
        let address = listener.local_addr().unwrap();
        let expected = (0..CHUNK_SIZE * 2 + 17)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let response_body = expected.clone();
        let server = thread::spawn(move || {
            for expected_method in ["HEAD", "GET"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.starts_with(expected_method));

                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                    response_body.len()
                )
                .unwrap();
                if expected_method == "GET" {
                    stream.write_all(&response_body).unwrap();
                }
            }
        });

        let client = Client::builder().no_proxy().build().unwrap();
        let mut source = HttpStream::with_client(
            format!("http://{address}/audio"),
            client,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert_eq!(source.should_get_chunk(), (true, 0));
        source.try_write_chunk(true);
        assert_eq!(source.should_get_chunk(), (false, expected.len()));
        let mut actual = Vec::new();
        source.read_to_end(&mut actual).unwrap();

        assert_eq!(actual, expected);
        server.join().unwrap();
    }
}
