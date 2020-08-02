# ductile

[![Rust](https://github.com/edomora97/ductile/workflows/Rust/badge.svg?branch=master)](https://github.com/edomora97/ductile/actions?query=workflow%3ARust)
[![Audit](https://github.com/edomora97/ductile/workflows/Audit/badge.svg?branch=master)](https://github.com/edomora97/ductile/actions?query=workflow%3AAudit)
[![crates.io](https://img.shields.io/crates/v/ductile.svg)](https://crates.io/crates/ductile)
[![Docs](https://docs.rs/ductile/badge.svg)](https://docs.rs/ductile)

A channel implementation that allows both local in-memory channels and remote TCP-based channels
with the same interface.

## Components

This crate exposes an interface similar to `std::sync::mpsc` channels. It provides a multiple
producers, single consumer channel that can use under the hood local in-memory channels
(provided by `crossbeam_channel`) but also network channels via TCP sockets. The remote
connection can also be encrypted using ChaCha20.

Like `std::sync::mpsc`, there could be more `ChannelSender` but there can be only one
`ChannelReceiver`. The two ends of the channel are generic over the message type sent but the
type must match (this is checked at compile time only for local channels). If the types do not
match errors will be returned and possibly a panic can occur since the channel breaks.

The channels also offer a _raw_ mode where the data is not serialized and send as-is,
improving drastically the performances for unstructured data. It should be noted that you can
mix the two modes in the same channel but you must be careful to always receive with the correct
mode (you cannot receive raw data with the normal `recv` method). Extra care should be taken
when cloning the sender and using it from more threads.

With remote channels the messages are serialized using `bincode`.

## Usage

Here there are some simple examples of how you can use this crate:

### Simple local channel
```rust
let (tx, rx) = new_local_channel();
tx.send(42u64).unwrap();
tx.send_raw(&vec![1, 2, 3, 4]).unwrap();
let answer = rx.recv().unwrap();
assert_eq!(answer, 42u64);
let data = rx.recv_raw().unwrap();
assert_eq!(data, vec![1, 2, 3, 4]);
```

### Local channel with custom data types
Note that your types must be `Serialize` and `Deserialize`.
```rust
#[derive(Serialize, Deserialize)]
struct Thing {
    pub x: u32,
    pub y: String,
}
let (tx, rx) = new_local_channel();
tx.send(Thing {
    x: 42,
    y: "foobar".into(),
})
.unwrap();
let thing: Thing = rx.recv().unwrap();
assert_eq!(thing.x, 42);
assert_eq!(thing.y, "foobar");
```

### Remote channels
```rust
let port = 18452; // let's hope we can bind this port!
let mut server = ChannelServer::bind(("127.0.0.1", port)).unwrap();

// this examples need a second thread since the handshake cannot be done using a single thread
// only
let client_thread = std::thread::spawn(move || {
    let (sender, receiver) = connect_channel(("127.0.0.1", port)).unwrap();

    sender.send(vec![1, 2, 3, 4]).unwrap();

    let data: Vec<i32> = receiver.recv().unwrap();
    assert_eq!(data, vec![5, 6, 7, 8]);

    sender.send(vec![9, 10, 11, 12]).unwrap();
    sender.send_raw(&vec![1, 2, 3, 4, 5, 6, 7, 8, 9]).unwrap();
});

let (sender, receiver, _addr) = server.next().unwrap();
let data: Vec<i32> = receiver.recv().unwrap();
assert_eq!(data, vec![1, 2, 3, 4]);

sender.send(vec![5, 6, 7, 8]).unwrap();

let data = receiver.recv().unwrap();
assert_eq!(data, vec![9, 10, 11, 12]);
let data = receiver.recv_raw().unwrap();
assert_eq!(data, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
```

### Remote channel with encryption
```rust
let port = 18453;
let enc_key = [69u8; 32];
let mut server = ChannelServer::bind_with_enc(("127.0.0.1", port), enc_key).unwrap();

let client_thread = std::thread::spawn(move || {
    let (sender, receiver) = connect_channel_with_enc(("127.0.0.1", port), &enc_key).unwrap();

    sender.send(vec![1u8, 2, 3, 4]).unwrap();

    let data: Vec<u8> = receiver.recv().unwrap();
    assert_eq!(data, vec![5u8, 6, 7, 8]);

    sender.send(vec![69u8; 12345]).unwrap();
    sender.send_raw(&vec![1, 2, 3, 4, 5, 6, 7, 8, 9]).unwrap();
});

let (sender, receiver, _addr) = server.next().unwrap();

let data: Vec<u8> = receiver.recv().unwrap();
assert_eq!(data, vec![1u8, 2, 3, 4]);

sender.send(vec![5u8, 6, 7, 8]).unwrap();

let data = receiver.recv().unwrap();
assert_eq!(data, vec![69u8; 12345]);
let file = receiver.recv_raw().unwrap();
assert_eq!(file, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
```

## Protocol

### Messages
All the _normal_ (non-raw) messages are encapsulated inside a `ChannelMessage::Message`,
serialized and sent normally.

The messages in raw mode are sent differently depending if the channel is local or remote.
If the channel is local there is no serialization penality so the data is simply sent into the
channel. If the channel is removed to avoid serialization a small message with the data length
is sent first, followed by the actual payload (that can be eventually encrypted).

### Handshakes
Local channels do not need an handshake, therefore this section refers only to remote channels.

There are 2 kinds of handshake: one for encrypted channels and one for non-encrypted ones.
The porpuse of the handshake is to share encryption information (like the nonce) and check if
the encryption key is correct.

For encrypted channels 2 rounds of handshakes take place:

- both parts generate 12 bytes of cryptographically secure random data (the channel nonce) and
  send them to the other party unencrypted.
- after receiving the channel nonce each party encrypts a known contant (a magic number of 4
  bytes) and sends it back to the other.
- when those 4 bytes are received they get decrypted and if they match the initial magic number
  the key is _probably_ valid and the handshake completes.

For unencrytpted channels the same handshake is done but with a static key and nonce and only
the magic is encrypted. All the following messages will be sent unencrypted.

License: MIT
