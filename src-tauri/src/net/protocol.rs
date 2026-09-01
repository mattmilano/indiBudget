//! What the two sides say to each other.
//!
//! Strict request/reply: the client sends one message and reads one reply
//! before sending another. There is no pipelining and no server-initiated
//! traffic, which keeps a connection impossible to desynchronise.

use serde::{Deserialize, Serialize};

use crate::boundary::{Request, Response};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Offer a pairing proof and ask to be added as a known machine.
    Pair { proof: String, label: String },
    /// Present the machine's token and a person's credentials.
    Authenticate {
        device_token: String,
        login: String,
        password: String,
    },
    /// Run a boundary command. Only valid once signed in.
    Invoke { request: Request },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Pairing succeeded; here is the machine's token. Sent exactly once —
    /// the host keeps only its hash.
    Paired { device_token: String },
    /// Sign-in succeeded.
    Authenticated {
        display_name: String,
        is_owner: bool,
    },
    /// A boundary reply.
    Reply { response: Response },
    /// Anything refused, with the sentence to show the person.
    Refused {
        sentence: String,
        /// Set when the refusal was a throttle, so the client can say how long.
        retry_after_secs: Option<u64>,
    },
}

impl ServerMessage {
    pub fn refused(sentence: impl Into<String>) -> Self {
        ServerMessage::Refused {
            sentence: sentence.into(),
            retry_after_secs: None,
        }
    }

    pub fn throttled(sentence: impl Into<String>, seconds: u64) -> Self {
        ServerMessage::Refused {
            sentence: sentence.into(),
            retry_after_secs: Some(seconds),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn client_messages_round_trip() {
        let messages = vec![
            ClientMessage::Pair {
                proof: "abc123".into(),
                label: "Alex's laptop".into(),
            },
            ClientMessage::Authenticate {
                device_token: "token".into(),
                login: "alex".into(),
                password: "Password1".into(),
            },
            ClientMessage::Invoke {
                request: Request::new("get_accounts", json!({})),
            },
        ];

        for message in messages {
            let encoded = serde_json::to_string(&message).unwrap();
            let decoded: ClientMessage = serde_json::from_str(&encoded).unwrap();
            assert_eq!(
                serde_json::to_string(&decoded).unwrap(),
                encoded,
                "message did not survive the round trip"
            );
        }
    }

    #[test]
    fn server_messages_round_trip() {
        let messages = vec![
            ServerMessage::Paired {
                device_token: "token".into(),
            },
            ServerMessage::Authenticated {
                display_name: "Alex".into(),
                is_owner: false,
            },
            ServerMessage::Reply {
                response: Response::ok(json!({ "ok": true })),
            },
            ServerMessage::refused("Nope."),
            ServerMessage::throttled("Too many tries.", 30),
        ];

        for message in messages {
            let encoded = serde_json::to_string(&message).unwrap();
            let decoded: ServerMessage = serde_json::from_str(&encoded).unwrap();
            assert_eq!(serde_json::to_string(&decoded).unwrap(), encoded);
        }
    }

    #[test]
    fn a_password_is_never_part_of_a_server_message() {
        // Credentials travel client to host only. Nothing the host says back
        // should be able to carry one, even by accident.
        let encoded = serde_json::to_string(&ServerMessage::Authenticated {
            display_name: "Alex".into(),
            is_owner: false,
        })
        .unwrap();
        assert!(!encoded.contains("password"));
    }
}
