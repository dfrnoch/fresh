use serde::{Deserialize, Serialize};

/// `SndOp` enum represents on eof the `Room` operator subcommands.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum SndOp<'a> {
  /** Open the current `Room`, allowing in the general public. */
  Open,
  /** Close the current `Room` to anyone not specifically `Invite`d. */
  Close,
  /** Ban the `User` with the supplied name from the `Room` (even if it's
  `Open`), removing him if he's currently in it. */
  Kick(&'a str),
  /** Allow the named `User` to enter the current room, even if it's `Close`d.
  Also sends an invitation message to the invited `User`. */
  Invite(&'a str),
  /** Transfer operatorship to another `User`. (The `User` must be in the
  `Room` in order to receive the mantle of ophood. */
  Give(&'a str),
}

/** The `Sndr` enum is the structure that gets serialized to JSON and passed
along the TCP connections between the server and the various clients.

The first four variants, `Text {...}`, `Ping`, `Priv {...}` and `Logout(...)`
are bi-directional, being used to send similar information both from the
client to the server and vice-versa.

The next six, `Name`, `Join`, `Query`, `Block`, `Unblock, and `Op`, are
for sending commands or requests from the client to the server.

The final three, `Info`, `Err`, and `Misc` are used only to send information
from the server back to the client.
*/
#[derive(Clone, Copy, Debug, Serialize)]
pub enum Sndr<'a> {
  //
  // Bi-directional messages
  //
  /** Typical chunk of text exchanged while chatting. */
  Text { who: &'a str, lines: &'a [&'a str] },

  /** Request for of acknowledgement of proof of connection.

  If the server hasn't received any data from the client in a while, it
  will send one of these. The client can then respond with one to indicate
  it's still connected.
  */
  Ping,

  /** A private message, delivered only to a single recipient.

  When sent client to server, the `who` member should identify the
  _recipient_; when sent server to client, the `who` member identifies
  the _source_.
  */
  Priv { who: &'a str, text: &'a str },

  /** A message from the client indicating it would like to disconnect
  cleanly from the server (displaying the supplied message to other `User`s
  in the `Room`); in response, the server will send back one of these as
  an acknowledgement to close the connection.

  The server may also send one of these to notify the client that it is
  being logged out without the client requesting it, like upon an error.
  */
  Logout(&'a str),

  //
  // Client-to-server messages
  //
  /** Name change request. */
  Name(&'a str),

  /** Request to join (creating if necessary) a room. */
  Join(&'a str),

  /** A request from the client to the server for some type of information,
  like a list of users matching a pattern.

  Currently-supported `Query` types are:

  ``` ignore
  // Request client's address as seen by the server
  Query {
      what: "addr",
      arg: "",        // unused for this query
  };

  // Request the list of names of `User`s in the current `Room`.
  Query {
      what: "roster",
      arg: "",        // unused for this query
  };

  // Request for a list of `User` ID strs that begin with the supplied
  // pattern.
  Query {
      what: "who",
      arg: "xxxhead", // pattern to match case-and-whitespace-insensitively
  };

  // Similar to the `"who"` request but for `Room` ID strings.
  Query {
      what: "rooms",
      arg: "froglovers", // pattern to match (as before)
  };
  ```
  */
  Query { what: &'a str, arg: &'a str },

  /** Request from the client to block messages (including private messages)
  from the `User` with the matching name. */
  Block(&'a str),

  /** Client request to unblock the given user. */
  Unblock(&'a str),

  /** One of the operator subcommands (see the `SndOp` enum). */
  Op(SndOp<'a>),

  //
  // Server-to-client messages
  //
  /** A non-error, miscellaneously-informative message sent from the
  server to the client.
  */
  Info(&'a str),

  /** A messgae the client indicating the client has done something wrong,
  like send an invalid message.
  */
  Err(&'a str),

  /** The `Misc` variant represents information that the client may want
  to display in a structured manner (and not just as an unadorned line of
  text). For any given "type" of `Misc` message, the client is free to either
  implement its own form of displaying the information, or to just use
  the contents of the provided `.alt` field.

  Current `Misc` variants (with example field values):

  ``` ignore
  // in response to a `Query { what: "roster", ... }`
  Misc {
      what: "roster",
      data: &["user1", "user2", "user7"], # ...
      alt:  "Room Some Room: user1 (operator) user2, user7...",
  };

  // when a user joins a `Room`
  Misc {
      what: "join",
      data: &["Some Dude", "Some Room"],
      alt:  "Some Dude joins Some Room.",
  };

  // when a user logs out or otherwise leaves a `Room`
  Misc {
      what: "leave",
      data: &["Some Dude", "[ moved to another room]"],
      alt:  "Some Dude moved to another room.",
  };

  // when a user is kicked from the current `Room`
  Misc {
      what: "kick_other",
      data: &["Annoying Guy", "Some Room"],
      alt:  "Annoying Guy has been kicked from Some Room.",
  };

  // when _you_ are kicked from the current `Room`
  Misc {
      what: "kick_you",
      data: &["Some Room"],
      alt:  "You have been kicked from Some Room.",
  };

  // when a user changes his or her name
  Misc {
      what: "name",
      data: &["Guy's Old Name", "Guy's New Name"],
      alt:  "\"Guy's Old Name\" is now known as \"Guy's New Name\".",
  };

  // when the `Room` operator changes
  Misc {
      what: "new_op",
      data: &["Some Other Gal", "Some Room"],
      alt:  "Some Other Gal is now the operator of Some Room.",
  };

  // in response to a `Query { what: "addr", ... }`
  Misc {
      what: "addr",
      data: &["127.0.0.1:12345", ],
      alt:  "Your public address is 127.0.0.1:12345.",
  };

  // in response to a `Query { what: "who", arg: "head", }`
  Misc {
      what: "who",
      data: &["Headmaster", "Head5h0t 360 420 69", "heading home | Fred"],
      alt:  "Matching names: headmaster, head5h0t36042069, headinghome|fred",
  };

  // in response to a `Query { what: "rooms", arg: "gay", }`
  Misc {
      what: "rooms",
      data: &["Some Room", "Gay Hamster Fan Club",
              "Gay Rights Advocacy", "Gaystation 3"],
      alt:  "Matching Rooms: gayspacecommunism, gayhamsterfanclub, gayrightsadvocacy, gaystation3",
  };

  // echoes a `Priv` back to the sender
  Misc {
      what: "priv_echo",
      data: &["Some Other User", "Don't listen to these jerks, they're not actually communists."],
      alt:  "$ You @ Some Other User: Don't listen to these jerks, they're not actually communists.",
  };
  ```
  */
  Misc {
    what: &'a str,
    data: &'a [&'a str],
    alt: &'a str,
  },
}

impl Sndr<'_> {
  /** Return the JSON-encoded bytes of the reciever. */
  pub fn bytes(&self) -> Vec<u8> {
    serde_json::to_vec_pretty(&self).unwrap()
  }
}

/** The data-owning counterpart to `SndOp` that gets _deserialized_.

    `&str`s have become `String`s, but their meanings are identical.
*/
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum RcvOp {
  Open,
  Close,
  Kick(String),
  Invite(String),
  Give(String),
}

/** The data-owning counterpart to `Sndr` that gets _deserialized_.
All `&str`s become `String`s and `&[&str]`s become `Vec<String>`s.
Otherwise, their structures and meanings are otherwise almost identical.
*/
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

/** Specifies an endpoint for an `Env` (below). Used in routing and
blocking messages.
*/
#[derive(Clone, Copy, Debug)]
pub enum End {
  /// a single user of the given user ID
  User(u64),
  /// an entire `Room` with the given room ID (most messages are like this)
  Room(u64),
  /// the server (only messages _from_ the server)
  Server,
  //TODO: a message to all users
  All,
}

/** An `Env` (-elope) wraps the bytes of a JSON-encoded `Sndr`, along with
unambiguous source and destination information. This metadata is necessary
because the encoded message is opaque to the server without decoding it.
*/
#[derive(Clone, Debug)]
pub struct Env {
  pub source: End,
  pub dest: End,
  data: Vec<u8>,
}

impl<'a> Env {
  /** Encode and wrap a `Sndr`. */
  pub fn new(from: End, to: End, msg: &'a Sndr) -> Env {
    Env {
      source: from,
      dest: to,
      data: msg.bytes(),
    }
  }

  /** Get a reference to the encoded bytes. */
  pub fn bytes(&self) -> &[u8] {
    &self.data
  }

  /** Consume the `Env`, returning the underlying vector of bytes. */
  pub fn into_bytes(self) -> Vec<u8> {
    self.data
  }
}
