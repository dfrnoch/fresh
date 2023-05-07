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
default config dir. You'll probably want to at least set the server address. The
config options are

- `address`: This specifies the IP address and port of the server that the
  client should connect to. To override the default value, use the `-a`
  command-line option.

- `name`: This sets the desired name to use when connecting to the server. If
  the chosen name is already in use, a generic name will be assigned instead.
  The default value can be changed using the `-n` command-line option.

- `timeout_ms`: This sets the minimum amount of time (in milliseconds) that each
  iteration of the main loop should take. Decreasing this value can make the
  client feel more responsive, but may increase resource usage.

- `read_size`: This determines the number of bytes to read from the server
  during each iteration of the main loop. The default value is typically
  sufficient. Setting this to a very low number will affect performance, and
  setting it to 0 will make the client unusable.

- `roster_width`: This specifies the width (in characters) of the panel that
  displays the current room's roster. As the server limits usernames to 24
  characters, this setting is set to a reasonable default value.

- `cmd_char`: This is the character used to indicate that a line of input should
  be interpreted as a command rather than plain text. The default value will be
  assumed in the following instructions.

- `max_scrollback`: This sets the maximum number of lines to keep in the
  scrollback buffer.

- `min_scrollback`: This specifies the minimum number of lines to retain when
  the scrollback buffer reaches its maximum capacity. It should be noted that
  this value must be smaller than `max_scrollback`.

#### Use

The client's operation is _modal_. When you first start the client, you will be
in _insert_ mode (indicated by `Ins` in the lower-left-hand corner). In this
mode, you can type text, which will be sent to the server when you hit `Enter`.

Pressing `Esc` will put you in _command_ mode (indicated by `Cmd` in the
lower-left-hand corner). In this mode, you can enter either commands to the
client or and vi\
following keybindings are available:

- `q` will quit the client.

- `PgUp/PgDn` will scroll the chat text up/down one screen.

- The up/down arrow keys (or K/J) will scroll the chat text up/down one line.

- `alt-Up/Dn/PgUp/PgDn` will scroll the roster window.

- `h/l` will scroll the chat text left/right.

- `w/b` will scroll the chat text forward/backward one word.

- `D` will put you in _delete_ mode, in which you can delete characters from the
  current line using common vi keybindings. ex: `dw` will delete the word under
  the cursor.

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
