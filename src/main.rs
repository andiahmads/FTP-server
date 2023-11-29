use std::fs::{read_dir, Metadata};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::str;

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
#[allow(dead_code)]
enum ResultCode {
    RestartMarkerReply = 110,
    ServiceReadInXXXMinutes = 120,
    DataConnectionAlreadyOpen = 125,
    FileStatusOk = 150,
    Ok = 200,
    CommandNotImplementedSuperfluousAtThisSite = 202,
    SystemStatus = 211,
    DirectoryStatus = 212,
    FileStatus = 213,
    HelpMessage = 214,
    SystemType = 215,
    ServiceReadyForNewUser = 220,
    ServiceClosingControlConnection = 221,
    DataConnectionOpen = 225,
    ClosingDataConnection = 226,
    EnteringPassiveMode = 227,
    UserLoggedIn = 230,
    RequestedFileActionOkay = 250,
    PATHNAMECreated = 257,
    UserNameOkayNeedPassword = 331,
    NeedAccountForLogin = 332,
    RequestedFileActionPendingFurtherInformation = 350,
    ServiceNotAvailable = 421,
    CantOpenDataConnection = 425,
    ConnectionClosed = 426,
    FileBusy = 450,
    LocalErrorInProcessing = 451,
    InsufficientStorageSpace = 452,
    UnknownCommand = 500,
    InvalidParameterOrArgument = 501,
    CommandNotImplemented = 502,
    BadSequenceOfCommands = 503,
    CommandNotImplementedForThatParameter = 504,
    NotLoggedIn = 530,
    NeedAccountForStoringFiles = 532,
    FileNotFound = 550,
    PageTypeUnknown = 551,
    ExceededStorageAllocation = 552,
    FileNameNotAllowed = 553,
}

#[derive(Clone, Debug)]
enum Command {
    Auth,
    List,
    // Cwd(PathBuf),
    Syst, //implemantation command
    NoOp,
    Pwd,
    Type, //Anda dapat mentransfer data dengan extensi yang berbeda.
    Pasv,
    Unknown(String), //Jika perintah tersebut tidak ada (atau kita belum mengimplementasikannya belum diimplementasikan), Unknown akan dikembalikan dengan nama perintah.
    User(String),
}

// Dalam contoh ini, as_ref digunakan untuk mendapatkan referensi ke string yang sesuai dengan masing-masing varian enum.
// Pada varian Unknown, kita menggunakan as_str() untuk mendapatkan referensi ke string yang terdapat di dalam Unknown
impl AsRef<str> for Command {
    fn as_ref(&self) -> &str {
        match *self {
            Command::Auth => "AUTH",
            Command::List => "LIST",
            // Command::Cwd(_) => "CWD",
            Command::Syst => "SYST",
            Command::NoOp => "NOOP",
            Command::Pwd => "PWD",
            Command::Type => "TYPE",
            Command::Pasv => "PASV",
            Command::Unknown(_) => "UNKW",
            Command::User(_) => "USER",
        }
    }
}

impl Command {
    pub fn new(input: Vec<u8>) -> io::Result<Self> {
        // Pertama, kita membuat sebuah iterator untuk membagi vektor kita, sehingga kita dapat memisahkan
        // dari argumen-argumennya:
        let mut iter = input.split(|&byte| byte == b' ');

        // Kemudian, kita mendapatkan perintah:
        let mut command = iter.next().expect("command in input").to_vec();
        to_uppercase(&mut command);

        // Selanjutnya, kita mendapatkan argumen dengan memanggil next pada iterator iterator:
        let data = iter.next();
        let command = match command.as_slice() {
            b"AUTH" => Command::Auth,
            b"SYST" => Command::Syst,
            b"USER" => Command::User(
                data.map(|bytes| {
                    String::from_utf8(bytes.to_vec()).expect("cannot convert bytes to string")
                })
                .unwrap_or_default()
                .to_owned(),
            ),
            s => Command::Unknown(str::from_utf8(s).unwrap_or("").to_owned()),
        };
        Ok(command)
    }
}

fn to_uppercase(data: &mut [u8]) {
    for byte in data {
        if *byte >= 'a' as u8 && *byte <= 'z' as u8 {
            *byte -= 32;
        }
    }
}

// Sekarang kita dapat menulis fungsi untuk membaca data dari klien:
fn read_all_message(stream: &mut TcpStream) -> Vec<u8> {
    let buf = &mut [0; 1];
    let mut out = Vec::with_capacity(100);

    // infinity loop
    loop {
        match stream.read(buf) {
            Ok(received) if received > 0 => {
                if out.is_empty() && buf[0] == b' ' {
                    continue;
                }
                out.push(buf[0])
            }
            _ => return Vec::new(),
        }

        let len = out.len();
        if len > 1 && out[len - 2] == b'\r' && out[len - 1] == b'\n' {
            out.pop();
            out.pop();
            return out;
        }
    }
}

#[allow(dead_code)]
struct Client {
    cwd: PathBuf, //adalah singkatan dari direktori kerja saat ini stream adalah soket klien
    stream: TcpStream, // socket client
    name: Option<String>, //pengguna yang Anda dapatkan dari autentikasi pengguna
    data_writer: Option<TcpStream>,
}

impl Client {
    fn new(stream: TcpStream) -> Client {
        Client {
            cwd: PathBuf::from("/"), // root dir
            stream: stream,
            name: None,
            data_writer: None,
        }
    }

    fn handle_cmd(&mut self, cmd: Command) {
        println!("========> {:?}", cmd);
        match cmd {
            Command::Auth => send_cmd(
                &mut self.stream,
                ResultCode::CommandNotImplemented,
                "Not Implemented",
            ),
            Command::NoOp => send_cmd(&mut self.stream, ResultCode::Ok, "Doing nothing..."),

            Command::Syst => send_cmd(&mut self.stream, ResultCode::Ok, "I won't tell"),

            Command::Pwd => {
                let msg = format!("{}", self.cwd.to_str().unwrap_or(""));
                if !msg.is_empty() {
                    let message = format!("\"/{}\"", msg);
                    send_cmd(
                        &mut self.stream,
                        ResultCode::PATHNAMECreated,
                        &format!("\"/{}\" ", msg),
                    );
                } else {
                    send_cmd(
                        &mut self.stream,
                        ResultCode::FileNotFound,
                        "no such file or directory",
                    )
                }
            }

            Command::Type => send_cmd(
                &mut self.stream,
                ResultCode::Ok,
                "Transfer type changed successfully",
            ),
            Command::Pasv => {
                if self.data_writer.is_some() {
                    send_cmd(
                        &mut self.stream,
                        ResultCode::DataConnectionAlreadyOpen,
                        "already listening....",
                    )
                } else {
                    // Jika kita sudah memiliki koneksi data dengan klien ini, kita tidak perlu membuka yang baru, jadi kita tidak perlu melakukan apa pun:
                    let port = 43210;
                    send_cmd(
                        &mut self.stream,
                        ResultCode::EnteringPassiveMode,
                        &format!("127.0.0.1,{},{}", port >> 8, port & 0xFF),
                    );
                    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
                    let listener = TcpListener::bind(&addr).unwrap();
                    match listener.incoming().next() {
                        Some(Ok(client)) => {
                            self.data_writer = Some(client);
                        }
                        _ => send_cmd(
                            &mut self.stream,
                            ResultCode::ServiceNotAvailable,
                            "issue happend...",
                        ),
                    }
                }
            }

            Command::List => {
                if let Some(ref mut data_writer) = self.data_writer {
                    let mut tmp = PathBuf::from(".");
                    send_cmd(
                        &mut self.stream,
                        ResultCode::DataConnectionAlreadyOpen,
                        "starting to list directory.....",
                    );

                    let mut out = String::new();
                    for entry in read_dir(tmp).unwrap() {
                        for entry in dir {
                            if let Ok(entry) = entry {
                                add_file_info(entry.path(), &mut out);
                            }
                        }
                        send_data(data_writer, &out);
                    }
                } else {
                    send_cmd(
                        &mut self.stream,
                        ResultCode::ConnectionClosed,
                        "No opened data connection",
                    );
                }
                if self.data_writer.is_some() {
                    self.data_writer = None;
                    send_cmd(
                        &mut self.stream,
                        ResultCode::ClosingDataConnection,
                        "Transfer done",
                    );
                }
            }

            Command::User(username) => {
                if username.is_empty() {
                    send_cmd(
                        &mut self.stream,
                        ResultCode::InvalidParameterOrArgument,
                        "invalid username",
                    )
                } else {
                    self.name = Some(username.to_owned());
                    send_cmd(
                        &mut self.stream,
                        ResultCode::UserLoggedIn,
                        &format!("welcome {}!", username),
                    )
                }
            }
            Command::Unknown(s) => send_cmd(
                &mut self.stream,
                ResultCode::UnknownCommand,
                &format!("command {} not Implemented", s),
            ),
        }
    }
}

fn send_data(stream: &mut TcpStream, s: &str) {}

fn add_file_info(path: PathBuf, out: &mut str) {
    let extra = if path.is_dir() { "/" } else { "" };
    let is_dir = if path.is_dir() { "d" } else { "-" };

    let meta = match ::std::fs::metadata(&path) {
        Ok(meta) => meta,
        _ => return,
    };

    let (time, file_size) = get_file_info(&meta);
    let path = match path.to_str() {
        Some(path) => match path.split("/").last() {
            Some(path) => path,
            _ => return,
        },
        _ => return,
    };

    let right = if meta.permissions().readonly() {
        "r--r--r--"
    } else {
        "rw-rw-rw-"
    };
}

fn get_file_info(meta: &Metadata) {}
// Sekarang saatnya memperbarui fungsi handle_client:
fn handle_client(mut stream: TcpStream) {
    println!("new client connected!!");
    send_cmd(
        &mut stream,
        ResultCode::ServiceReadyForNewUser,
        "Welcome to this Rust FTP",
    );

    // let client = Client::new
}

fn send_cmd(stream: &mut TcpStream, code: ResultCode, message: &str) {
    let msg = if message.is_empty() {
        format!("{}\r\n", code as u32)
    } else {
        format!("{} {}\r\n", code as u32, message)
    };
    println!("<========= {}", msg);
    write!(stream, "{}", msg).unwrap();
}

fn main() {
    let listner = TcpListener::bind("0.0.0.0:1234").expect("Couldn't bind this address");

    println!("Waiting for clients to connect....");

    for stream in listner.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("New client!");
                if let Err(_) = stream.write(b"hello") {
                    println!("Failed to send hello... :'(");
                }
            }
            _ => {
                println!("A client tried to connect");
            }
        }
    }
}
