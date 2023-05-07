# Fresh

a chat server/client made in Rust

### Overview

The `fresh` project comprises a minimal chat server and client implemented in
Rust. The chat server exposes a simple JSON-based API, intended to facilitate
the development of feature-rich chat clients in any programming language.

### Installation

Head over to the [releases](https://github.com/lnxcz/frsh/releases) page and
download the latest release for your platform. Extract the archive and run the
binary.

### Client instructions

#### Configuration

Run `fresh -g` to generate a default config file; this will save it to the
dafault config dir. You'll probably want to at least set the server address. The
config options (and their default values) are

- `address = '127.0.0.1:1234'` The IP address and port of the server to which
  you'd like to connect. This can be overridden with the `-a` command-line
  option.

- `name = 'fresh user'` The name you'd like to (attempt to) use when connecting
  to the server. If a sufficiently similar name is taken, you will be renamed to
  something generic (but unique). This can be overridden with the `-n`
  command-line option.

- `timeout_ms = 100` Minimum amount of time (in milliseconds) per iteration of
  the main loop. Setting this to a smaller value will make typing feel snappier,
  but may consume more system resources.

- `read_size = 1024` The the amount (in bytes) the client attempts to read from
  its connection with the server each time through the main loop. The default
  amount is almost undoubtedly fine. Setting this to a very low number will
  impact your experience; setting this to 0 will render the client inoperable.

- `roster_width = 24` Number of characters wide to draw the panel that holds the
  current `Room`'s roster. By default, the server limits user names to 24
  characters, so this is a reasonable width.

- `cmd_char = ';'` The character prepended to an input line to indicate to the
  client it should be interpreted as a _command_, rather than just as text to be
  sent to the chat. These instructions will assume the default value.

- `max_scrollback = 2000` The maximum number of lines to keep in the scrollback
  buffer.

- `min_scrollback = 1000` When the scrollback buffer reaches `max_scrollback`,
  it will be trimmed to this many lines. For reasons that should be obvious,
  this must be smaller than `max_scrollback`.

#### Use

The client's operation is _modal_. When you first start the client, you will be
in _insert_ mode (indicated by `Ins` in the lower-left-hand corner). In this
mode, you can type text, which will be sent to the server when you hit `Enter`.
The following keybindings are available:

- `Backspace`, `Delete`, `Home`, `End`, and `Left`/`Right` (arrow keys) act as
  you'd expect

- `alt-Backspace` and `alt-Delete` will delete an entire word

- `alt-Left` and `alt-Right` will move the cursor position by one word

Hitting escape (or backspace when the input line is empty) will put you in
_command_ mode (indicated by `Com` in the lower-left-hand corner), where you
will eventually be able to control more aspects of the client. Right now,

- `q` will quit the client.

- `PgUp/PgDn` will scroll the chat text up/down one screen.

- The up/down arrow keys (or K/J) will scroll the chat text up/down one line.

- `alt-Up/Dn/PgUp/PgDn` will scroll the roster window.

You can also use the following commands:

- `;quit [message]` will quit the client, sending the optional message to the
  room.

- `;name <new_username>` will change your name to something stupid.

- `;join <room>` will join the room named `<room>` or create it if it doesn't
  exist.

- `;priv <user> <message>` will send a private message to the user. The user can
  be in any room.

- `;who [user]` will show you a list of all users in the server, or all users
  whose name matches `[user]` (if `[user]` is provided).

- `;rooms [room]` will show you a list of all rooms on the server, or all rooms
  whose name matches `[room]` (if `[room]` is provided).

- `;block <user>` will block all incoming messages from the user.

- `;unblock <user>` will unblock a blocked user.

If you are the Operator of a Room, you can also use the following commands:

- `;op close` will "close" an open room, preventing anyone without an explicit
  invitation from entering.

- `;op open` will "open" a closed room, allowing eeryone to enter.

- `;op invite <user>` will send an invitation to the user whose name matches
  `<user>` (if that user exists).

- `;op kick <user>` will kick the user.

- `;op ban <user>` will ban the user and prevent them from rejoining the room.

- `;op give <user>` will give Operator privileges to the user whose name matches
  `<user>` (if that user exists).

### Server Instructions

Once you start the server for the first time, it will create a `config.toml` in
the OS-specific configuration directory. On Linux, this is
`~/.config/fresh-server/config.toml`. You can edit this file to change the
server's behavior. The default values are:

```toml
address = "192.168.1.13:51516"
tick_ms = 500
blackout_to_ping_ms  = 10000
blackout_to_kick_ms  = 20000
max_user_name_length = 24
max_room_name_length = 32
lobby_name = 'Lobby'
welcome = "Welcome to the server."
log_file = 'freshd.log'
log_level = 1
byte_limit = 512
bytes_per_tick = 6
```

### TODO (server):

- IP specific blocks/bans. Server doesn't really know anything about the
  client's IP address.

### TODO (client):

- Show if user is OP in roster
