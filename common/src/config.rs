use std::fmt::Write;
use std::path::PathBuf;
use std::time::Duration;

use simplelog::LevelFilter;

const CLIENT_NAME: &str = "fresh.toml";
const SERVER_NAME: &str = "freshd.toml";

//  Default values
const ADDR: &str = "127.0.0.1:51516"; // server address
const SERVER_LOG: &str = "freshd.log"; // server log file
const NAME: &str = "fresh user"; // client user name
const LOBBY_NAME: &str = "Lobby"; // server landing room name
const WELCOME: &str = "Welcome to the server."; // server welcome message
const SERVER_TICK: u64 = 500; // server, min time through main loop
const BYTE_LIMIT: usize = 512; // server user rate limiting byte quota
const BYTE_TICK: usize = 6; // server byte quota dissipation per tick
const LOG_LEVEL: LevelFilter = LevelFilter::Warn; // server log level
const BLACKOUT_TO_PING: u64 = 10000; /* msec since data received from a client that server will send a ping */
const BLACKOUT_TO_KICK: u64 = 20000; /* to confirm connection or log the client off for unreachability */
const CLIENT_TICK: u64 = 100; // client time through main loop
const READ_SIZE: usize = 1024; // client number of bytes per read attempt
const ROSTER_WIDTH: u16 = 24; // Also server max user name and max room name lengths
const CMD_CHAR: char = '/';
const MIN_SCROLLBACK: usize = 1000; // client `Line`s of scrollback kept
const MAX_SCROLLBACK: usize = 2000; // client will trim scrollback to MIN_SCROLLBACK when this many `Line`s reached

/// Generates a platform-specific path
fn default_config_dir() -> PathBuf {
  match directories::BaseDirs::new() {
    None => PathBuf::default(),
    Some(d) => d.config_dir().to_path_buf(),
  }
}

/** Attempt to read from a series of files, returning the contents of the
first successful attempt.
*/
fn read_first_to_string(ps: &[PathBuf]) -> Result<String, String> {
  let mut misses = String::from("Couldn't read from");
  for p in ps.iter() {
    match std::fs::read_to_string(p) {
      Ok(s) => {
        return Ok(s);
      }
      Err(e) => {
        write!(&mut misses, "\n\"{}\" ({})", p.display(), e).unwrap();
      }
    }
  }
  Err(misses)
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ServerConfigFile {
  address: Option<String>,
  tick_ms: Option<u64>,
  blackout_to_ping_ms: Option<u64>,
  blackout_to_kick_ms: Option<u64>,
  max_user_name_length: Option<usize>,
  max_room_name_length: Option<usize>,
  lobby_name: Option<String>,
  welcome: Option<String>,
  log_file: Option<String>,
  log_level: Option<u8>,
  byte_limit: Option<usize>,
  bytes_per_tick: Option<usize>,
}

#[derive(Debug)]
pub struct ServerConfig {
  pub address: String,
  pub min_tick: Duration,
  pub blackout_time_to_ping: Duration,
  pub blackout_time_to_kick: Duration,
  pub max_user_name_length: usize,
  pub max_room_name_length: usize,
  pub lobby_name: String,
  pub welcome: String,
  pub log_file: String,
  pub log_level: LevelFilter,
  pub byte_limit: usize,
  pub byte_tick: usize,
}

impl ServerConfig {
  pub fn configure() -> ServerConfig {
    let mut pathz: Vec<PathBuf> = Vec::new();
    pathz.push(PathBuf::from(SERVER_NAME));
    {
      let mut p = default_config_dir();
      p.push(SERVER_NAME);
      pathz.push(p);
    }

    let cfgf: ServerConfigFile = match read_first_to_string(&pathz) {
      Ok(s) => match toml::from_str(&s) {
        Ok(x) => x,
        Err(e) => {
          println!("Error parsing config file: {}", &e);
          std::process::exit(1);
        }
      },
      Err(e) => {
        println!("Error reading config file: {}", &e);
        println!("Using default configuration.");
        ServerConfigFile::default()
      }
    };

    let logl: LevelFilter = match cfgf.log_level {
      None => LOG_LEVEL,
      Some(0) => LevelFilter::Off,
      Some(1) => LevelFilter::Error,
      Some(2) => LevelFilter::Warn,
      Some(3) => LevelFilter::Info,
      Some(4) => LevelFilter::Debug,
      Some(5) => LevelFilter::Trace,
      Some(_) => {
        println!("Invalid log level in config file.");
        LevelFilter::Trace
      }
    };

    ServerConfig {
      address: cfgf.address.unwrap_or_else(|| ADDR.to_string()),
      min_tick: Duration::from_millis(cfgf.tick_ms.unwrap_or_else(|| SERVER_TICK)),
      blackout_time_to_ping: Duration::from_millis(
        cfgf.blackout_to_ping_ms.unwrap_or_else(|| BLACKOUT_TO_PING),
      ),
      blackout_time_to_kick: Duration::from_millis(
        cfgf.blackout_to_kick_ms.unwrap_or_else(|| BLACKOUT_TO_KICK),
      ),
      max_user_name_length: cfgf
        .max_user_name_length
        .unwrap_or_else(|| ROSTER_WIDTH as usize),
      max_room_name_length: cfgf
        .max_room_name_length
        .unwrap_or_else(|| ROSTER_WIDTH as usize),
      lobby_name: cfgf.lobby_name.unwrap_or_else(|| LOBBY_NAME.to_string()),
      welcome: cfgf.welcome.unwrap_or_else(|| WELCOME.to_string()),
      log_file: cfgf.log_file.unwrap_or_else(|| SERVER_LOG.to_string()),
      log_level: logl,
      byte_limit: cfgf.byte_limit.unwrap_or_else(|| BYTE_LIMIT),
      byte_tick: cfgf.bytes_per_tick.unwrap_or_else(|| BYTE_TICK),
    }
  }
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ClientConfigFile {
  address: Option<String>,
  name: Option<String>,
  timeout_ms: Option<u64>,
  read_size: Option<usize>,
  roster_width: Option<u16>,
  cmd_char: Option<char>,
  max_scrollback: Option<usize>,
  min_scrollback: Option<usize>,
}

#[derive(Debug)]
pub struct ClientConfig {
  pub address: String,
  pub name: String,
  pub tick: Duration,
  pub read_size: usize,
  pub roster_width: u16,
  pub cmd_char: char,
  pub max_scrollback: usize,
  pub min_scrollback: usize,
}

impl ClientConfig {
  pub fn configure(path: Option<String>) -> Result<ClientConfig, String> {
    let mut pathz: Vec<PathBuf> = Vec::new();
    if let Some(p) = path {
      pathz.push(PathBuf::from(&p));
    }
    pathz.push(PathBuf::from(CLIENT_NAME));
    {
      let mut p = default_config_dir();
      p.push(CLIENT_NAME);
      pathz.push(p);
    }

    let f: ClientConfigFile = match read_first_to_string(&pathz) {
      Ok(s) => match toml::from_str(&s) {
        Ok(x) => x,
        Err(e) => {
          return Err(format!("Error parsing config file: {}", &e));
        }
      },
      Err(e) => {
        println!("Error reading config file: {}", &e);
        println!("Using default configuration.");
        ClientConfigFile::default()
      }
    };

    let max_scroll = f.max_scrollback.unwrap_or_else(|| MAX_SCROLLBACK);
    let min_scroll = f.min_scrollback.unwrap_or_else(|| MIN_SCROLLBACK);
    let cmd_char = f.cmd_char.unwrap_or_else(|| CMD_CHAR);

    if max_scroll < min_scroll {
      return Err("max_scrollback cannot be smaller than min_scrollback".to_string());
    };
    if (cmd_char as u32) > 128 {
      return Err("cmd_char must be an ASCII character".to_string());
    };

    let cc = ClientConfig {
      address: f.address.unwrap_or_else(|| String::from(ADDR)),
      name: f.name.unwrap_or_else(|| String::from(NAME)),
      tick: Duration::from_millis(f.timeout_ms.unwrap_or_else(|| CLIENT_TICK)),
      read_size: f.read_size.unwrap_or_else(|| READ_SIZE),
      roster_width: f.roster_width.unwrap_or_else(|| ROSTER_WIDTH),
      cmd_char,
      max_scrollback: max_scroll,
      min_scrollback: min_scroll,
    };

    Ok(cc)
  }

  pub fn generate() -> Result<String, String> {
    let cfg = ClientConfigFile {
      address: Some(String::from(ADDR)),
      name: Some(String::from(NAME)),
      timeout_ms: Some(CLIENT_TICK),
      read_size: Some(READ_SIZE),
      roster_width: Some(ROSTER_WIDTH),
      cmd_char: Some(CMD_CHAR),
      max_scrollback: Some(MAX_SCROLLBACK),
      min_scrollback: Some(MIN_SCROLLBACK),
    };

    let mut cfg_path = default_config_dir();
    cfg_path.push(CLIENT_NAME);
    let cfg_str = toml::to_string(&cfg).unwrap();

    match std::fs::write(&cfg_path, cfg_str) {
      Ok(()) => match cfg_path.to_str() {
        Some(x) => Ok(String::from(x)),
        None => Ok(cfg_path.to_string_lossy().to_string()),
      },
      Err(e) => Err(format!(
        "Error writing new config file {}: {}",
        &cfg_path.display(),
        &e
      )),
    }
  }
}
