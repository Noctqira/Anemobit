//! Test doubles and helpers (compiled only for tests; never touches the network).

mod fake_agent;
mod fake_v3_agent;
mod mock_transport;
mod test_clock;

pub(crate) use fake_agent::FakeAgent;
pub(crate) use fake_v3_agent::FakeV3Agent;
pub(crate) use mock_transport::{MockTransport, response, response_with};
pub(crate) use test_clock::TestClock;

use std::future::Future;

/// Converts a hex string (may contain spaces) to bytes.
pub(crate) fn hex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// Runs a future to completion on a new glib main context.
pub(crate) fn run<F: Future>(future: F) -> F::Output {
    glib::MainContext::new().block_on(future)
}
