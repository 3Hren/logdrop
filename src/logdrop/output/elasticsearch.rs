use time;

use std::io::timer::Timer;
use std::sync::Arc;
use std::time::Duration;

use logdrop::Payload;
use logdrop::logger::{Debug, Info, Warn};

use url::Url;
use http::client::RequestWriter;
use http::method::Post;

use super::Output;

enum Event {
    Chunk(String),
    Timeout,
}

pub struct ElasticsearchOutput {
    tx: Sender<Event>,
}

impl ElasticsearchOutput {
    pub fn new(host: &str, port: u16) -> ElasticsearchOutput {
        let (tx, rx) = channel();
        let output = ElasticsearchOutput {
            tx: tx.clone(),
        };

        let (timer_tx, timer_rx) = channel();
        spawn(proc(){
            let duration = Duration::milliseconds(3000);
            let mut timer = Timer::new().unwrap();
            loop {
                log!(Debug, "Output::ES" -> "waiting for {}ms timeout", 3000u32);
                let timeout = timer.oneshot(duration);

                select! {
                    () = timer_rx.recv() => {},
                    () = timeout.recv()  => { tx.send(Timeout); }
                }
            }
        });

        let base = format!("{}:{}", host, port);
        spawn(proc(){
            let base = base;
            let limit = 100;
            let mut queue: Vec<String> = Vec::new();

            // All settings.
            loop {
                match rx.recv() {
                    Chunk(chunk) => {
                        queue.push(chunk);
                        if queue.len() >= limit {
                            timer_tx.send(());

                            ElasticsearchOutput::send(base.as_slice(), ElasticsearchOutput::make_body(&queue));
                            queue.clear();
                        }
                    }
                    Timeout      => {
                        log!(Debug, "Output::ES" -> "timed out");
                        ElasticsearchOutput::send(base.as_slice(), ElasticsearchOutput::make_body(&queue));
                        queue.clear();
                    }
                }
            }
        });

        output
    }

    fn make_body(queue: &Vec<String>) -> Arc<String> {
        let mut data = String::new();
        for item in queue.iter() {
            data.push_str("{\"index\":{}}\n");
            data.push_str(item.as_slice());
            data.push_str("\n");
        }
        Arc::new(data)
    }

    fn send(base: &str, data: Arc<String>) {
        if data.is_empty() {
            return
        }

        log!(Debug, "Output::ES" -> "emitting");

        let url = format!("http://{}/logs/log3/_bulk", base);
        let url = match Url::parse(url.as_slice()) {
            Ok(url)  => url,
            Err(err) => {
                log!(Warn, "Output::ES" -> "failed to parse '{}' - {}", url, err);
                return;
            }
        };

        log!(Debug, "Output::ES" -> "sending bulk index request at {}", url);
        spawn(proc(){
            let mut request: RequestWriter = match RequestWriter::new(Post, url) {
                Ok(request) => request,
                Err(err)    => {
                    log!(Warn, "Output::ES" -> "failed to build POST request - {}", err);
                    return;
                }
            };

            request.headers.content_length = Some(data.len());
            match request.write(data.as_bytes()) {
                Ok(())   => {}
                Err(err) => {
                    log!(Warn, "Output::ES" -> "failed to write payload - {}", err);
                    return;
                }
            }

            let response = match request.read_response() {
                Ok(response)  => response,
                Err((_, err)) => {
                    log!(Warn, "Output::ES" -> "failed to perform POST request - {}", err);
                    return;
                }
            };
            log!(Debug, "Output::ES" -> "ok - {}", response.status);
        });
    }
}

impl Output for ElasticsearchOutput {
    fn feed(&mut self, payload: &Payload) {
        self.tx.send(Chunk(payload.to_string()));
    }
}
