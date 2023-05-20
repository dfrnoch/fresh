use serde::{Deserialize, Serialize};

/// `SndOp` enum represents one of the `Room` operator subcommands.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum SndOp<'a> {
    /// Open the current `Room`, allowing the general public to join.
    Open,
    /// Close the current `Room`, restricting access to only those with invitations.
    Close,
    /// Ban the specified `User` from the `Room` (even if it's `Open`), and remove them if they're currently in it.
    Kick(&'a str),
    /// Permit the specified `User` to enter the current room (even if it's `Close`d), and send an invitation message to them.
    Invite(&'a str),
    /// Transfer operator privileges to another `User` (the `User` must be in the `Room` to receive the privileges).
    Give(&'a str),
}

/// The `Sndr` enum is the structure that gets serialized to JSON and sent over TCP connections between the server and clients.
///
/// The first four variants (`Text {...}`, `Ping`, `Priv {...}`, and `Logout(...)`) are bi-directional.
///
/// The next six (`Name`, `Join`, `Query`, `Block`, `Unblock`, and `Op`) are for sending commands or requests from the client to the server.
///
/// The final three (`Info`, `Err`, and `Misc`) are used only to send information from the server back to the client.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum Sndr<'a> {
    // Bi-directional messages
    /// Standard text message exchanged in a chat.
    Text { who: &'a str, lines: &'a [&'a str] },

    /// Ping message to confirm the connection between client and server.
    Ping,

    /// Private message sent to a single recipient.
    Priv { who: &'a str, text: &'a str },

    /// Client message to request a clean disconnect from the server, displaying the supplied message to other users in the `Room`. The server responds with a similar message as an acknowledgment before closing the connection.
    Logout(&'a str),

    // Client-to-server messages
    /// Request to change the user's name.
    Name(&'a str),

    /// Request to join (or create if necessary) a room.
    Join(&'a str),

    /// A request from the client to the server for specific information.
    Query { what: &'a str, arg: &'a str },

    /// Request from the client to block messages (including private messages) from the specified `User`.
    Block(&'a str),

    /// Request from the client to unblock the specified `User`.
    Unblock(&'a str),

    /// One of the operator subcommands (refer to the `SndOp` enum).
    Op(SndOp<'a>),

    // Server-to-client messages
    /// A non-error informative message sent from the server to the client.
    Info(&'a str),

    /// A message sent from the server to the client indicating an error or an invalid action.
    Err(&'a str),

    /// The `Misc` variant represents information that the client may want to display in a structured manner.
    /// The client can either implement its own way of displaying the information or use the provided `.alt` field.
    Misc {
        what: &'a str,
        data: &'a [&'a str],
        alt: &'a str,
    },
}

impl Sndr<'_> {
    /// Return the JSON-encoded bytes of the reciever.
    pub fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&self).unwrap()
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum RcvOp {
    Open,
    Close,
    Kick(String),
    Invite(String),
    Give(String),
}

/// The reciever side of the client-server protocol.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Rcvr {
    Text {
        #[serde(default)]
        who: String,
        lines: Vec<String>,
    },

    Ping,
    Priv {
        who: String,
        text: String,
    },
    Logout(String),

    Name(String),
    Join(String),
    Query {
        what: String,
        arg: String,
    },
    Block(String),
    Unblock(String),
    Op(RcvOp),

    Info(String),
    Err(String),
    Misc {
        what: String,
        data: Vec<String>,
        alt: String,
    },
}

impl Rcvr {
    pub fn counts(&self) -> bool {
        matches!(
            self,
            Rcvr::Text { who: _, lines: _ }
                | Rcvr::Priv { who: _, text: _ }
                | Rcvr::Name(_)
                | Rcvr::Join(_)
        )
    }
}

/// Message endpoint.
#[derive(Clone, Copy, Debug)]
pub enum End {
    User(u64),
    Room(u64),
    Server,
    All,
}

/// The `Env` struct represents a message sent between two `End`s.
#[derive(Clone, Debug)]
pub struct Env {
    pub source: End,
    pub dest: End,
    data: Vec<u8>,
}

impl<'a> Env {
    pub fn new(from: End, to: End, msg: &'a Sndr) -> Env {
        Env {
            source: from,
            dest: to,
            data: msg.bytes(),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}
