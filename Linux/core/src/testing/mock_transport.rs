use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::rc::Rc;
use std::task::{Poll, Waker};

use crate::VarBind;
use crate::snmp::message::SnmpMessage;
use crate::snmp::pdu::{Pdu, PduType};
use crate::transport::Transport;

use super::{FakeAgent, FakeV3Agent};

/// Decides which datagrams to return on each send: arguments are the sent packet and the send count (starting at 1).
type Responder = Box<dyn FnMut(&[u8], usize) -> Vec<Vec<u8>>>;

/// Test transport. Clones share state, so the test can still inspect it after handing it to the client.
#[derive(Clone)]
pub(crate) struct MockTransport {
    state: Rc<RefCell<State>>,
}

struct State {
    responder: Option<Responder>,
    sent: Vec<Vec<u8>>,
    queue: VecDeque<Vec<u8>>,
    waker: Option<Waker>,
    send_error: Option<String>,
    incoming_error: Option<String>,
}

impl MockTransport {
    /// Responds with raw bytes.
    pub(crate) fn raw(responder: impl FnMut(&[u8], usize) -> Vec<Vec<u8>> + 'static) -> Self {
        MockTransport {
            state: Rc::new(RefCell::new(State {
                responder: Some(Box::new(responder)),
                sent: Vec::new(),
                queue: VecDeque::new(),
                waker: None,
                send_error: None,
                incoming_error: None,
            })),
        }
    }

    /// Responds per v2c message (packets that can't be decoded get no reply).
    pub(crate) fn v2c(
        mut responder: impl FnMut(&SnmpMessage, usize) -> Vec<Vec<u8>> + 'static,
    ) -> Self {
        Self::raw(move |datagram, count| match SnmpMessage::decode(datagram) {
            Ok(request) => responder(&request, count),
            Err(_) => Vec::new(),
        })
    }

    /// Never answers any request.
    pub(crate) fn silent() -> Self {
        Self::raw(|_, _| Vec::new())
    }

    /// `FakeAgent` answers every v2c request.
    pub(crate) fn with_agent(agent: FakeAgent) -> Self {
        Self::v2c(move |request, _| vec![agent.respond(request).encode()])
    }

    /// `FakeV3Agent` answers every request.
    pub(crate) fn with_v3_agent(agent: FakeV3Agent) -> Self {
        Self::raw(move |datagram, _| agent.respond(datagram))
    }

    /// Number of handles to the shared state (to confirm the client was dropped and the connection closed).
    pub(crate) fn handle_count(&self) -> usize {
        Rc::strong_count(&self.state)
    }

    /// Sent packets.
    pub(crate) fn sent_raw(&self) -> Vec<Vec<u8>> {
        self.state.borrow().sent.clone()
    }

    /// Sent v2c messages.
    pub(crate) fn sent(&self) -> Vec<SnmpMessage> {
        self.sent_raw()
            .iter()
            .filter_map(|d| SnmpMessage::decode(d).ok())
            .collect()
    }

    /// All later sends fail.
    pub(crate) fn fail_sends(&self, message: &str) {
        self.state.borrow_mut().send_error = Some(message.into());
    }

    /// Simulates a connection failure: after the queue is drained, every receive fails.
    pub(crate) fn fail_incoming(&self, message: &str) {
        let mut state = self.state.borrow_mut();
        state.incoming_error = Some(message.into());
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    /// Puts a received packet in directly.
    pub(crate) fn deliver(&self, datagram: Vec<u8>) {
        let mut state = self.state.borrow_mut();
        state.queue.push_back(datagram);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl Transport for MockTransport {
    async fn send(&self, datagram: &[u8]) -> io::Result<()> {
        let (mut responder, count) = {
            let mut state = self.state.borrow_mut();
            state.sent.push(datagram.to_vec());
            if let Some(message) = &state.send_error {
                return Err(io::Error::other(message.clone()));
            }
            (state.responder.take(), state.sent.len())
        };
        // Don't hold the borrow while the responder runs, so it can call MockTransport methods again.
        let replies = responder
            .as_mut()
            .map(|r| r(datagram, count))
            .unwrap_or_default();
        self.state.borrow_mut().responder = responder;
        for reply in replies {
            self.deliver(reply);
        }
        Ok(())
    }

    async fn receive(&self) -> io::Result<Vec<u8>> {
        std::future::poll_fn(|cx| {
            let mut state = self.state.borrow_mut();
            if let Some(datagram) = state.queue.pop_front() {
                return Poll::Ready(Ok(datagram));
            }
            if let Some(message) = &state.incoming_error {
                return Poll::Ready(Err(io::Error::other(message.clone())));
            }
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await
    }
}

/// A normal response to `request`.
pub(crate) fn response(request: &SnmpMessage, var_binds: Vec<VarBind>) -> Vec<u8> {
    response_with(request, request.pdu.request_id, 0, 0, var_binds)
}

/// A response with a chosen request-id and error-status / error-index.
pub(crate) fn response_with(
    request: &SnmpMessage,
    request_id: i32,
    error_status: i64,
    error_index: i64,
    var_binds: Vec<VarBind>,
) -> Vec<u8> {
    let pdu = Pdu {
        pdu_type: PduType::Response,
        request_id,
        error_status,
        error_index,
        var_binds,
    };
    SnmpMessage::new(request.community.clone(), pdu).encode()
}
