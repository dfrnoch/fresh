car# `fresh`

an IRC-like chat server/client in Rust

### Overview

`fresh` is a simple IRC-like chat server with a simple, easily-extensible
JSON-based protocol. The `freshd` server is meant to be small and easy to
configure. There is a "reference" client, `fresh`, also written in Rust, but the
protocol is meant to be sufficiently simple that clients (with a variety of
features) should be easily implementable in any number of languages.

### State

Both `freshd` and `fresh` work, at least on Debian 10 and Windows 10, and, I
suspect, any vaguely POSIX-y system that sports Rust's `cargo`. There are still
features as yet to be implemented, like rate limiting and more effective
blocking (See the TODO sections, below, for more details.)

### Installation

Clone the repo and everything should `cargo build --release` just fine. This
will build both the `fresh` client and the `freshd` server.

### Client instructions

#### Configuration

Run `fresh -g` to generate a default config file; this will probably show up at
`your_normal_config_dir/fresh.toml`, but it might not. In any case, a message
will print with the path of the new file. The defaults are sane, but you'll
probably want to at least set the server address. The config options (and their
default values) are

- `address = '127.0.0.1:51516'` The IP address and port of the server to which
  you'd like to connect. This can be overridden with the `-a` command-line
  option.

- `name = 'fresh user'` The name you'd like to (attempt to) use when connecting
  to the server. If a sufficiently similar name is taken, you will be renamed to
  something generic (but unique). This can be overridden with the `-n`
  command-line option.

- `timeout_ms = 100` Minimum amount of time (in milliseconds) per iteration of
  the main loop. Setting this to a smaller value will make typing feel snappier,
  but may consume more system resources.

- `block_ms = 5000` I don't think this currently has a function.

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

There is also an optional `[colors]` stanza. The default colors work well for
some terminal color schemes, but not others, so this allows you to customize the
client so that it looks reasonable.

There are three types of text the client will show:

- normal text, which is displayed in the colors your terminal is already using
- dim text, which by default is high-intensity black on whatever background
  color your terminal is using
- highlighted text, which by default is high-intensity white on whatever
  background colr your teminal is using

The stanza then looks something like this:

```toml
[colors]
dim_foreground = 8
dim_background = 0
highlight_foreground = 15
highlight_background = 0
underline_as_bold = false
```

Where each of the color value is
[a number from 0-255](https://jonasjacek.github.io/colors/), and the
`underline_as_bold` option sets whether the terminal should attempt to use
underlining anywhere it would otherwise attempt to use bold text (as bold
doesn't show up well or render properly on some systems). Omitting any of these
values will default to normal terminal coloring.

#### Use

The client's operation is _modal_ (_a la_ `vi`). Because I am more sympathetic
than Bill Joy, the client launches in _input_ mode, where text you type gets
added to the input line and sent to the server when you hit return. This is
indicated by the `Ipt` in the lower-left-hand corner. In this mode,

- `Backspace`, `Delete`, `Home`, `End`, and `Left`/`Right` (arrow keys) act as
  you'd expect

- `alt-Backspace` and `alt-Delete` will delete an entire word

- `alt-Left` and `alt-Right` will move the cursor position by one word

Hitting escape (or backspace when the input line is empty) will put you in
_command_ mode (indicated by `Com` in the lower-left-hand corner), where you
will eventually be able to control more aspects of the client. Right now,

- `q` will quit with no leave message.

- `ctrl-q` will force-quit the client without sending any messages to the server
  to disconnect cleanly. The server will eventually figure this out when the
  client stops responding to `Msg::Ping`s, and log you out "posthumously".

- `PgUp/PgDn` will scroll the chat text up/down one screen.

- The up/down arrow keys will scroll the chat text up/down one line.

- `alt-Up/Dn/PgUp/PgDn` will scroll the roster window.

You can also type some server-interaction commands from input mode. For example,

- `;quit Y'all're losers!1` will disconnect from the server, showing the
  message, "Y'all're losers!1" to everyone in the Room.

- `;name xXx_h34d5h0t_420_xXx` will change your name to something stupid.

- `;join Tracks of the World` will join the room called "Tracks of the World",
  creating it if it doesn't exist. (Creation of a room also sets the creator as
  that room's "Operator", although this currently bestows no special
  priviliges.)

- `;priv somedude Come join tracksoftheworld.` will send the message "Come join
  tracksoftheworld" to the user whose name matches `somedude` (if that user
  exists).

- `;who xxx` will request a list of all connected users whose names start with a
  case-and-whitespace-insensitive match of `xxx`. A plain `;who` with no text to
  match will return a list of all users.

- `;rooms xxx` will, like `;who` above, request a list of all extant Room names
  that begin with a case-and-whitespace-insensitive match of `xxx`. A plain
  `;rooms` with no text to match will return a list of all Rooms.

- `;block jerkuser` Will block the user whose name currently matches "jerkuser"
  (if not already blocked).

- `;unblock jerkuser` Will unblock same, if blocked.

In addition if you are the Room operator, you have several more commands
available:

- `;op close` will "close" the room, preventing anyone who hasn't been
  explicitly `invite`d (see below) from entering.

- `;op open` will "open" a closed room, again allowing the general public.

- `;op invite somebody` will send an invitation message to user `somebody`, as
  well as permitting them to join an otherwise "closed" room.

- `;op kick somebody`
- `;op ban somebody` &mdash; Both of these commands function identically,
  removing `somebody` from the room (if present) and preventing him or her from
  entering in the future. This ban can be lifted by an explicit `invite`.

- `;op give somebody` will transfer the mantleship of operator to user
  `somebody`.

### A note about user and room names

Names are allowed to contain any arbitrary unicode characters, including
whitespace (although they cannot be made of _only_ whitespace).

### Server Instructions

The server configuration on my machine is at `~/.config/freshd/freshd.toml`; it
will probably be similarly placed on yours. You should make its contents look
something like this:

```toml
address = "192.168.1.13:51516"
tick_ms = 500
blackout_to_ping_ms  = 10000
blackout_to_kick_ms  = 20000
max_user_name_length = 24
max_room_name_length = 32
lobby_name = 'Lobby'
welcome = "Welcome to a fresh server."
log_file = 'freshd.log'
log_level = 1
byte_limit = 512
bytes_per_tick = 6
```

although you may want to change the `address` value to match where you want your
server to bind.

You will want to run it with `nohup` if you don't want to babysit it:

```sh
you@your_machine:~/fresh $ nohup target/release/freshd &
```

and you may want to redirect `stdout` to a specific file.

### TODO (server):

- IP specific blocks/bans. Server doesn't
  really know anything about the client's IP address.

### TODO (client):

- ~~A bunch of command-mode functionality needs to be implemented, like
  done; some isn't.)~~ This is in pretty good shape as of 2021-01-31, so as I
  think of specific things, I'll add them to this list.

- `vi`-like search in the scrollback history

- Input line history? Like Up/Down should scroll through input 

I am happy to entertain feature requests, but simplicity is a goal.

### A final confession

Test coverage is poor. Some of the modules have tests for some of the functions
and methods. Mostly this is just because I am lazy, and _most_ of what
transpires between the various elements in this software is simple. Much of the
client functionality is poorly-tested because it's dificult to write tests for
terminal output---here, the tests tend to be things you look at and say, "Yeah,
that looks right."
