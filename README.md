# Fresh

[![DeepSource](https://app.deepsource.com/gh/lnxcz/fresh.svg/?label=active+issues&show_trend=true&token=CMLlIbDLbu5SNKhIR0MXQspB)](https://app.deepsource.com/gh/lnxcz/fresh/?ref=repository-badge)

### Overview

`Fresh` is a simple & customizable chat server and client. It is designed to be
easy to use and easy to extend. Both the server and client are written in Rust,
and the client uses the crossterm library for its UI.

### Installation

Head over to the [releases](https://github.com/lnxcz/fresh/releases) page and
download the latest release for your platform. Extract the archive and run the
binary.

### Client instructions

#### Configuration

Both server and client will check for a `config.toml` file in the OS-specific
default config dir and directory from which the binary was run. You'll probably
want to at least set the server address. The config options are:

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
  displays the current room's users. As the server limits usernames to 24
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

- `Up/Dn/K/J` will scroll the chat text up/down one line.

- `SHIFT-Up/Dn/K/J` will scroll the roster window.

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

Once you start the server for the first time, it will create a `freshd.toml` in
the OS-specific configuration directory. On Linux, for example, this will be
`~/.config/freshd.toml`. You can edit this file to change the server's behavior.
The default values are:

```toml
address = "192.168.1.13:51516"      # The address to listen on
tick_ms = 500                       # The number of milliseconds between ticks
time_to_ping_ms  = 10000            # The number of milliseconds between pings
time_to_kick_ms  = 20000            # The number of milliseconds before kicking a client for not responding to a ping
max_user_name_length = 24           # The maximum length of a username
max_room_name_length = 32           # The maximum length of a room name
lobby_name = 'Lobby'                # The name of the lobby
welcome_message = "Welcome!"        # The message sent to the client when they connect
log_file = 'freshd.log'             # The name of the log file
log_level = 1                       # The log level (0-5)
byte_limit = 512                    # The number of bytes allowed per quota
bytes_per_tick = 6                  # The number of bytes to add to the quota per tick
```

## Network Communication

The network layer of the chat application consists of the protocol and the
socket. It is responsible for managing the communication between the clients and
the server.

The protocol, defined in `proto.rs`, contains two main structures: `Sndr` and
`Rcvr`. These structures represent messages exchanged between the server and
clients. The `Sndr` enum is used for sending messages, while the `Rcvr` enum is
used for receiving messages. Some messages are bi-directional, meaning they can
be used in both directions, while others are specific to client-to-server or
server-to-client communication.

The `Socket` struct, defined in `socket.rs`, handles the underlying TCP stream
and provides methods to read data, write data, and handle incoming messages. The
`SocketError` struct represents errors that may occur during socket operations.
The `Socket` struct also offers methods to manage the read and write buffers,
set buffer sizes, send data in a blocking or non-blocking manner, and manage the
connection's state.

Together, the `Socket` and `proto` modules form the network layer of the chat
application, providing a robust and efficient way to exchange messages between
clients and the server.

### TODO (server):

- IP specific blocks/bans. Server doesn't really know anything about the
  client's IP address.

### TODO (client):

- Show if user is OP in roster
