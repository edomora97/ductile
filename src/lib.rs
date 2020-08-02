//! [![Rust](https://github.com/edomora97/ductile/workflows/Rust/badge.svg?branch=master)](https://github.com/edomora97/ductile/actions?query=workflow%3ARust)
//! [![Audit](https://github.com/edomora97/ductile/workflows/Audit/badge.svg?branch=master)](https://github.com/edomora97/ductile/actions?query=workflow%3AAudit)
//! [![crates.io](https://img.shields.io/crates/v/ductile.svg)](https://crates.io/crates/ductile)
//! [![Docs](https://docs.rs/ductile/badge.svg)](https://docs.rs/ductile)
//!
//! A channel implementation that allows both local in-memory channels and remote TCP-based channels
//! with the same interface.
//!
//! # Components
//!
//! This crate exposes an interface similar to `std::sync::mpsc` channels. It provides a multiple
//! producers, single consumer channel that can use under the hood local in-memory channels
//! (provided by `crossbeam_channel`) but also network channels via TCP sockets. The remote
//! connection can also be encrypted using ChaCha20.
//!
//! Like `std::sync::mpsc`, there could be more `ChannelSender` but there can be only one
//! `ChannelReceiver`. The two ends of the channel are generic over the message type sent but the
//! type must match (this is checked at compile time only for local channels). If the types do not
//! match errors will be returned and possibly a panic can occur since the channel breaks.
//!
//! The channels also offer a _raw_ mode where the data is not serialized and send as-is,
//! improving drastically the performances for unstructured data. It should be noted that you can
//! mix the two modes in the same channel but you must be careful to always receive with the correct
//! mode (you cannot receive raw data with the normal `recv` method). Extra care should be taken
//! when cloning the sender and using it from more threads.
//!
//! With remote channels the messages are serialized using `bincode`.
//!
//! # Usage
//!
//! Here there are some simple examples of how you can use this crate:
//!
//! ## Simple local channel
//! ```
//! # use ductile::new_local_channel;
//! let (tx, rx) = new_local_channel();
//! tx.send(42u64).unwrap();
//! tx.send_raw(&vec![1, 2, 3, 4]).unwrap();
//! let answer = rx.recv().unwrap();
//! assert_eq!(answer, 42u64);
//! let data = rx.recv_raw().unwrap();
//! assert_eq!(data, vec![1, 2, 3, 4]);
//! ```
//!
//! ## Local channel with custom data types
//! Note that your types must be `Serialize` and `Deserialize`.
//! ```
//! # use ductile::new_local_channel;
//! # use serde::{Serialize, Deserialize};
//! #[derive(Serialize, Deserialize)]
//! struct Thing {
//!     pub x: u32,
//!     pub y: String,
//! }
//! let (tx, rx) = new_local_channel();
//! tx.send(Thing {
//!     x: 42,
//!     y: "foobar".into(),
//! })
//! .unwrap();
//! let thing: Thing = rx.recv().unwrap();
//! assert_eq!(thing.x, 42);
//! assert_eq!(thing.y, "foobar");
//! ```
//!
//! ## Remote channels
//! ```
//! # use ductile::{ChannelServer, connect_channel};
//! let port = 18452; // let's hope we can bind this port!
//! let mut server = ChannelServer::bind(("127.0.0.1", port)).unwrap();
//!
//! // this examples need a second thread since the handshake cannot be done using a single thread
//! // only
//! let client_thread = std::thread::spawn(move || {
//!     let (sender, receiver) = connect_channel(("127.0.0.1", port)).unwrap();
//!
//!     sender.send(vec![1, 2, 3, 4]).unwrap();
//!
//!     let data: Vec<i32> = receiver.recv().unwrap();
//!     assert_eq!(data, vec![5, 6, 7, 8]);
//!
//!     sender.send(vec![9, 10, 11, 12]).unwrap();
//!     sender.send_raw(&vec![1, 2, 3, 4, 5, 6, 7, 8, 9]).unwrap();
//! });
//!
//! let (sender, receiver, _addr) = server.next().unwrap();
//! let data: Vec<i32> = receiver.recv().unwrap();
//! assert_eq!(data, vec![1, 2, 3, 4]);
//!
//! sender.send(vec![5, 6, 7, 8]).unwrap();
//!
//! let data = receiver.recv().unwrap();
//! assert_eq!(data, vec![9, 10, 11, 12]);
//! let data = receiver.recv_raw().unwrap();
//! assert_eq!(data, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
//! # client_thread.join().unwrap();
//! ```
//!
//! ## Remote channel with encryption
//! ```
//! # use ductile::{ChannelServer, connect_channel_with_enc};
//! let port = 18453;
//! let enc_key = [69u8; 32];
//! let mut server = ChannelServer::bind_with_enc(("127.0.0.1", port), enc_key).unwrap();
//!
//! let client_thread = std::thread::spawn(move || {
//!     let (sender, receiver) = connect_channel_with_enc(("127.0.0.1", port), &enc_key).unwrap();
//!
//!     sender.send(vec![1u8, 2, 3, 4]).unwrap();
//!
//!     let data: Vec<u8> = receiver.recv().unwrap();
//!     assert_eq!(data, vec![5u8, 6, 7, 8]);
//!
//!     sender.send(vec![69u8; 12345]).unwrap();
//!     sender.send_raw(&vec![1, 2, 3, 4, 5, 6, 7, 8, 9]).unwrap();
//! });
//!
//! let (sender, receiver, _addr) = server.next().unwrap();
//!
//! let data: Vec<u8> = receiver.recv().unwrap();
//! assert_eq!(data, vec![1u8, 2, 3, 4]);
//!
//! sender.send(vec![5u8, 6, 7, 8]).unwrap();
//!
//! let data = receiver.recv().unwrap();
//! assert_eq!(data, vec![69u8; 12345]);
//! let file = receiver.recv_raw().unwrap();
//! assert_eq!(file, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
//! # client_thread.join().unwrap();
//! ```
//!
//! # Protocol
//!
//! ## Messages
//! All the _normal_ (non-raw) messages are encapsulated inside a `ChannelMessage::Message`,
//! serialized and sent normally.
//!
//! The messages in raw mode are sent differently depending if the channel is local or remote.
//! If the channel is local there is no serialization penality so the data is simply sent into the
//! channel. If the channel is removed to avoid serialization a small message with the data length
//! is sent first, followed by the actual payload (that can be eventually encrypted).
//!
//! ## Handshakes
//! Local channels do not need an handshake, therefore this section refers only to remote channels.
//!
//! There are 2 kinds of handshake: one for encrypted channels and one for non-encrypted ones.
//! The porpuse of the handshake is to share encryption information (like the nonce) and check if
//! the encryption key is correct.
//!
//! For encrypted channels 2 rounds of handshakes take place:
//!
//! - both parts generate 12 bytes of cryptographically secure random data (the channel nonce) and
//!   send them to the other party unencrypted.
//! - after receiving the channel nonce each party encrypts a known contant (a magic number of 4
//!   bytes) and sends it back to the other.
//! - when those 4 bytes are received they get decrypted and if they match the initial magic number
//!   the key is _probably_ valid and the handshake completes.
//!
//! For unencrytpted channels the same handshake is done but with a static key and nonce and only
//! the magic is encrypted. All the following messages will be sent unencrypted.
#![deny(missing_docs)]
#![deny(missing_doc_code_examples)]

#[macro_use]
extern crate log;

use std::cell::RefCell;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use chacha20::stream_cipher::{NewStreamCipher, SyncStreamCipher};
use chacha20::{ChaCha20, Key, Nonce};
use crossbeam_channel::{unbounded, Receiver, Sender};
use failure::{bail, Error};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// A magic constant used to check the protocol integrity between two remote hosts.
const MAGIC: u32 = 0x69421997;
/// Wrapper to `std::result::Result` defaulting the error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Message wrapper that is sent in the channel. It allows serializing normal messages as well as
/// informing the other end that some raw data is coming in the channel and it should not be
/// deserialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum ChannelMessage<T> {
    /// The message is a normal application message of type T.
    Message(T),
    /// Message with some raw data. This message is used only in local channels.
    RawData(Vec<u8>),
    /// Message telling the other end that some raw data of the specified length is coming. This is
    /// used only in remote channels. The data is not included here avoiding unnecessarily
    /// serialization.
    /// If the channel uses encryption this value only informs about the length of the actual raw
    /// data, not the number of bytes sent into the channel. In fact the actual number of bytes sent
    /// is a bit larger (due to encryption overheads that can be eventually removed).
    RawDataStart(usize),
}

/// Actual `ChannelSender` implementation. This exists to hide the variants from the public API.
#[derive(Clone)]
enum ChannelSenderInner<T> {
    /// The connection is only a local in-memory channel.
    Local(Sender<ChannelMessage<T>>),
    /// The connection is with a remote party. The Arc<Mutex<>> is needed because TcpStream is not
    /// Clone and sending concurrently is not safe.
    Remote(Arc<Mutex<TcpStream>>),
    /// The connection is with a remote party, encrypted with ChaCha20.
    RemoteEnc(Arc<Mutex<(TcpStream, ChaCha20)>>),
}

/// The channel part that sends data. It is generic over the type of messages sent and abstracts the
/// underlying type of connection. The connection type can be local in memory, or remote using TCP
/// sockets.
///
/// This sender is Clone since the channel is multiple producers, single consumer, just like
/// `std::sync::mpsc`.
#[derive(Clone)]
pub struct ChannelSender<T> {
    inner: ChannelSenderInner<T>,
}

/// Actual `ChannelReceiver` implementation. This exists to hide the variants from the public API.
enum ChannelReceiverInner<T> {
    /// The connection is only a local in-memory channel.
    Local(Receiver<ChannelMessage<T>>),
    /// The connection is with a remote party over a TCP socket.
    Remote(RefCell<TcpStream>),
    /// The connection is with a remote party and it is encrypted using ChaCha20.
    RemoteEnc(RefCell<(TcpStream, ChaCha20)>),
}

/// The channel part that receives data. It is generic over the type of messages sent in the
/// channel. The type of the messages must match between sender and receiver.
///
/// This type is not Clone since the channel is multiple producers, single consumer, just like
/// `std::sync::mpsc`.
pub struct ChannelReceiver<T> {
    inner: ChannelReceiverInner<T>,
}

impl<T> ChannelSender<T>
where
    T: 'static + Send + Sync + Serialize,
{
    /// Serialize and send a message in the channel. The message is not actually serialized for
    /// local channels, but the type must be `Send + Sync`.
    ///
    /// For remote channel the message is serialized, if you want to send raw data (i.e. `[u8]`)
    /// that not need to be serialized consider using `send_raw` since its performance is way
    /// better.
    ///
    /// This method is guaranteed to fail if the receiver is dropped only for local channels. Using
    /// remote channels drops this requirement.
    ///
    /// ```
    /// # use ductile::new_local_channel;
    /// let (sender, receiver) = new_local_channel();
    /// sender.send(42u8).unwrap();
    /// drop(receiver);
    /// assert!(sender.send(69u8).is_err());
    /// ```
    pub fn send(&self, data: T) -> Result<()> {
        match &self.inner {
            ChannelSenderInner::Local(sender) => sender
                .send(ChannelMessage::Message(data))
                .map_err(|e| e.into()),
            ChannelSenderInner::Remote(sender) => {
                let mut sender = sender.lock().unwrap();
                let stream = sender.deref_mut();
                ChannelSender::<T>::send_remote_raw(stream, ChannelMessage::Message(data))
            }
            ChannelSenderInner::RemoteEnc(stream) => {
                let mut stream = stream.lock().unwrap();
                let (stream, enc) = stream.deref_mut();
                ChannelSender::<T>::send_remote_raw_enc(stream, enc, ChannelMessage::Message(data))
            }
        }
    }

    /// Send some raw data in the channel without serializing it. This is possible only for raw
    /// unstructured data (`[u8]`), but this methods is much faster than `send` since it avoids
    /// serializing the message.
    ///
    /// This method is guaranteed to fail if the receiver is dropped only for local channels. Using
    /// remote channels drops this requirement.
    ///
    /// ```
    /// # use ductile::new_local_channel;
    /// let (sender, receiver) = new_local_channel::<()>();
    /// sender.send_raw(&vec![1, 2, 3, 4]).unwrap();
    /// drop(receiver);
    /// assert!(sender.send_raw(&vec![1, 2]).is_err());
    /// ```
    pub fn send_raw(&self, data: &[u8]) -> Result<()> {
        match &self.inner {
            ChannelSenderInner::Local(sender) => {
                Ok(sender.send(ChannelMessage::RawData(data.into()))?)
            }
            ChannelSenderInner::Remote(sender) => {
                let mut sender = sender.lock().expect("Cannot lock ChannelSender");
                let stream = sender.deref_mut();
                ChannelSender::<T>::send_remote_raw(
                    stream,
                    ChannelMessage::RawDataStart(data.len()),
                )?;
                Ok(stream.write_all(&data)?)
            }
            ChannelSenderInner::RemoteEnc(stream) => {
                let mut stream = stream.lock().unwrap();
                let (stream, enc) = stream.deref_mut();
                ChannelSender::<T>::send_remote_raw_enc(
                    stream,
                    enc,
                    ChannelMessage::RawDataStart(data.len()),
                )?;
                let data = ChannelSender::<T>::encrypt_buffer(data.into(), enc)?;
                Ok(stream.write_all(&data)?)
            }
        }
    }

    /// Serialize and send a `ChannelMessage` data to the remote channel, without encrypting
    /// message.
    fn send_remote_raw(stream: &mut TcpStream, data: ChannelMessage<T>) -> Result<()> {
        Ok(bincode::serialize_into(stream, &data)?)
    }

    /// Serialize and send a `ChannelMessage` data to the remote channel, encrypting message.
    fn send_remote_raw_enc(
        stream: &mut TcpStream,
        encryptor: &mut ChaCha20,
        data: ChannelMessage<T>,
    ) -> Result<()> {
        let data = bincode::serialize(&data)?;
        let data = ChannelSender::<T>::encrypt_buffer(data, encryptor)?;
        stream.write_all(&data)?;
        Ok(())
    }

    /// Encrypt a buffer, including it's length into a new buffer.
    fn encrypt_buffer(mut data: Vec<u8>, encryptor: &mut ChaCha20) -> Result<Vec<u8>> {
        let mut res = Vec::from((data.len() as u32).to_le_bytes());
        res.append(&mut data);
        encryptor.apply_keystream(&mut res);
        Ok(res)
    }

    /// Given this is a remote channel, change the type of the message. Will panic if this
    /// is a local channel.
    ///
    /// This function is useful for implementing a protocol where the message types change during
    /// the execution, for example because initially there is an handshake message, followed by the
    /// actual protocol messages.
    ///
    /// ```
    /// # use ductile::{ChannelServer, connect_channel, ChannelSender};
    /// # let server = ChannelServer::<i32, i32>::bind("127.0.0.1:12358").unwrap();
    /// # let thread = std::thread::spawn(move || {
    /// #    let (sender, receiver) = connect_channel::<_, i32, i32>("127.0.0.1:12358").unwrap();
    /// #    assert_eq!(receiver.recv().unwrap(), 42i32);
    /// #    sender.send(69i32).unwrap();
    /// let sender: ChannelSender<i32> = sender.change_type();
    /// let sender: ChannelSender<String> = sender.change_type();
    /// # });
    /// # for (sender, receiver, address) in server {
    /// #    sender.send(42i32).unwrap();
    /// #    assert_eq!(receiver.recv().unwrap(), 69i32);
    /// #   break;
    /// # }
    /// # thread.join().unwrap();
    /// ```
    pub fn change_type<T2>(self) -> ChannelSender<T2> {
        match self.inner {
            ChannelSenderInner::Remote(r) => ChannelSender {
                inner: ChannelSenderInner::Remote(r),
            },
            ChannelSenderInner::RemoteEnc(r) => ChannelSender {
                inner: ChannelSenderInner::RemoteEnc(r),
            },
            ChannelSenderInner::Local(_) => panic!("Cannot change ChannelSender::Local type"),
        }
    }
}

impl<T> ChannelReceiver<T>
where
    T: 'static + DeserializeOwned,
{
    /// Receive a message from the channel. This method will block until the other end sends a
    /// message.
    ///
    /// If the other end used `send_raw` this method panics since the channel corrupts.
    ///
    /// ```
    /// # use ductile::new_local_channel;
    /// let (sender, receiver) = new_local_channel();
    /// sender.send(42);
    /// let num: i32 = receiver.recv().unwrap();
    /// assert_eq!(num, 42);
    /// ```
    pub fn recv(&self) -> Result<T> {
        let message = match &self.inner {
            ChannelReceiverInner::Local(receiver) => receiver.recv()?,
            ChannelReceiverInner::Remote(receiver) => ChannelReceiver::recv_remote_raw(receiver)?,
            ChannelReceiverInner::RemoteEnc(receiver) => {
                let mut receiver = receiver.borrow_mut();
                let (receiver, decryptor) = receiver.deref_mut();
                ChannelReceiver::recv_remote_raw_enc(receiver, decryptor)?
            }
        };
        match message {
            ChannelMessage::Message(mex) => Ok(mex),
            _ => panic!("Expected ChannelMessage::Message"),
        }
    }

    /// Receive some raw data from the channel. This method will block until the other end sends
    /// some data.
    ///
    /// If the other end used `send` this method panics since the channel corrupts.
    ///
    /// ```
    /// # use ductile::new_local_channel;
    /// let (sender, receiver) = new_local_channel::<()>();
    /// sender.send_raw(&vec![1, 2, 3]);
    /// let data: Vec<u8> = receiver.recv_raw().unwrap();
    /// assert_eq!(data, vec![1, 2, 3]);
    /// ```
    pub fn recv_raw(&self) -> Result<Vec<u8>> {
        match &self.inner {
            ChannelReceiverInner::Local(receiver) => match receiver.recv()? {
                ChannelMessage::RawData(data) => Ok(data),
                _ => panic!("Expected ChannelMessage::RawData"),
            },
            ChannelReceiverInner::Remote(receiver) => {
                match ChannelReceiver::<T>::recv_remote_raw(receiver)? {
                    ChannelMessage::RawDataStart(len) => {
                        let mut receiver = receiver.borrow_mut();
                        let mut buf = vec![0u8; len];
                        receiver.read_exact(&mut buf)?;
                        Ok(buf)
                    }
                    _ => panic!("Expected ChannelMessage::RawDataStart"),
                }
            }
            ChannelReceiverInner::RemoteEnc(receiver) => {
                let mut receiver = receiver.borrow_mut();
                let (receiver, decryptor) = receiver.deref_mut();
                match ChannelReceiver::<T>::recv_remote_raw_enc(receiver, decryptor)? {
                    ChannelMessage::RawDataStart(_) => {
                        let buf = ChannelReceiver::<T>::decrypt_buffer(receiver, decryptor)?;
                        Ok(buf)
                    }
                    _ => panic!("Expected ChannelMessage::RawDataStart"),
                }
            }
        }
    }

    /// Receive a message from the TCP stream of a channel.
    fn recv_remote_raw(receiver: &RefCell<TcpStream>) -> Result<ChannelMessage<T>> {
        let mut receiver = receiver.borrow_mut();
        Ok(bincode::deserialize_from(receiver.deref_mut())?)
    }

    /// Receive a message from the encrypted TCP stream of a channel.
    fn recv_remote_raw_enc(
        receiver: &mut TcpStream,
        decryptor: &mut ChaCha20,
    ) -> Result<ChannelMessage<T>> {
        let buf = ChannelReceiver::<T>::decrypt_buffer(receiver, decryptor)?;
        Ok(bincode::deserialize(&buf)?)
    }

    /// Receive and decrypt a frame from the stream, removing the header and returning the contained
    /// raw data.
    fn decrypt_buffer(receiver: &mut TcpStream, decryptor: &mut ChaCha20) -> Result<Vec<u8>> {
        let mut len = [0u8; 4];
        receiver.read_exact(&mut len)?;
        decryptor.apply_keystream(&mut len);
        let len = u32::from_le_bytes(len) as usize;

        let mut buf = vec![0u8; len];
        receiver.read_exact(&mut buf)?;
        decryptor.apply_keystream(&mut buf);
        Ok(buf)
    }

    /// Given this is a remote channel, change the type of the message. Will panic if this is a
    /// `ChannelReceiver::Local`.
    ///
    /// This function is useful for implementing a protocol where the message types change during
    /// the execution, for example because initially there is an handshake message, followed by the
    /// actual protocol messages.
    ///
    /// ```
    /// # use ductile::{ChannelServer, connect_channel, ChannelSender, ChannelReceiver};
    /// # let server = ChannelServer::<i32, i32>::bind("127.0.0.1:12358").unwrap();
    /// # let thread = std::thread::spawn(move || {
    /// #    let (sender, receiver) = connect_channel::<_, i32, i32>("127.0.0.1:12358").unwrap();
    /// #    assert_eq!(receiver.recv().unwrap(), 42i32);
    /// #    sender.send(69i32).unwrap();
    /// let receiver: ChannelReceiver<i32> = receiver.change_type();
    /// let receiver: ChannelReceiver<String> = receiver.change_type();
    /// # });
    /// # for (sender, receiver, address) in server {
    /// #    sender.send(42i32).unwrap();
    /// #    assert_eq!(receiver.recv().unwrap(), 69i32);
    /// #   break;
    /// # }
    /// # thread.join().unwrap();
    /// ```
    pub fn change_type<T2>(self) -> ChannelReceiver<T2> {
        match self.inner {
            ChannelReceiverInner::Local(_) => panic!("Cannot change ChannelReceiver::Local type"),
            ChannelReceiverInner::Remote(r) => ChannelReceiver {
                inner: ChannelReceiverInner::Remote(r),
            },
            ChannelReceiverInner::RemoteEnc(r) => ChannelReceiver {
                inner: ChannelReceiverInner::RemoteEnc(r),
            },
        }
    }
}

/// Make a new pair of `ChannelSender` / `ChannelReceiver` that use in-memory message communication.
///
/// ```
/// # use ductile::new_local_channel;
/// let (tx, rx) = new_local_channel();
/// tx.send(42u64).unwrap();
/// tx.send_raw(&vec![1, 2, 3, 4]).unwrap();
/// let answer = rx.recv().unwrap();
/// assert_eq!(answer, 42u64);
/// let data = rx.recv_raw().unwrap();
/// assert_eq!(data, vec![1, 2, 3, 4]);
/// ```
pub fn new_local_channel<T>() -> (ChannelSender<T>, ChannelReceiver<T>) {
    let (tx, rx) = unbounded();
    (
        ChannelSender {
            inner: ChannelSenderInner::Local(tx),
        },
        ChannelReceiver {
            inner: ChannelReceiverInner::Local(rx),
        },
    )
}

/// Listener for connections on some TCP socket.
///
/// The connection between the two parts is full-duplex and the types of message shared can be
/// different. `S` and `R` are the types of message sent and received respectively. When initialized
/// with an encryption key it is expected that the remote clients use the same key. Clients that use
/// the wrong key are disconnected during an initial handshake.
pub struct ChannelServer<S, R> {
    /// The actual listener of the TCP socket.
    listener: TcpListener,
    /// An optional ChaCha20 key to use to encrypt the communication.
    enc_key: Option<[u8; 32]>,
    /// Save the type of the sending messages.
    _sender: PhantomData<*const S>,
    /// Save the type of the receiving messages.
    _receiver: PhantomData<*const R>,
}

impl<S, R> ChannelServer<S, R> {
    /// Bind a TCP socket and create a new `ChannelServer`. Only proper sockets are supported (not
    /// Unix sockets yet). This method does not enable message encryption.
    ///
    /// ```
    /// # use ductile::{ChannelServer, connect_channel};
    /// let server = ChannelServer::<i32, i32>::bind("127.0.0.1:12357").unwrap();
    /// assert!(ChannelServer::<(), ()>::bind("127.0.0.1:12357").is_err()); // port already in use
    ///
    /// # let thread =
    /// std::thread::spawn(move || {
    ///     let (sender, receiver) = connect_channel::<_, i32, i32>("127.0.0.1:12357").unwrap();
    ///     assert_eq!(receiver.recv().unwrap(), 42i32);
    ///     sender.send(69i32).unwrap();
    /// });
    ///
    /// for (sender, receiver, address) in server {
    ///     sender.send(42i32).unwrap();
    ///     assert_eq!(receiver.recv().unwrap(), 69i32);
    /// #   break;
    /// }
    /// # thread.join().unwrap();
    /// ```
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<ChannelServer<S, R>> {
        Ok(ChannelServer {
            listener: TcpListener::bind(addr)?,
            enc_key: None,
            _sender: Default::default(),
            _receiver: Default::default(),
        })
    }

    /// Bind a TCP socket and create a new `ChannelServer`. All the data transferred within this
    /// socket is encrypted using ChaCha20 initialized with the provided key. That key should be the
    /// same used by the clients that connect to this server.
    ///
    /// ```
    /// # use ductile::{ChannelServer, connect_channel, connect_channel_with_enc};
    /// let key = [42; 32];
    /// let server = ChannelServer::<i32, i32>::bind_with_enc("127.0.0.1:12357", key).unwrap();
    /// assert!(ChannelServer::<(), ()>::bind("127.0.0.1:12357").is_err()); // port already in use
    ///
    /// # let thread =
    /// std::thread::spawn(move || {
    ///     let (sender, receiver) = connect_channel_with_enc::<_, i32, i32>("127.0.0.1:12357", &key).unwrap();
    ///     assert_eq!(receiver.recv().unwrap(), 42i32);
    ///     sender.send(69i32).unwrap();
    /// });
    ///
    /// for (sender, receiver, address) in server {
    ///     sender.send(42i32).unwrap();
    ///     assert_eq!(receiver.recv().unwrap(), 69i32);
    /// #   break;
    /// }
    /// # thread.join().unwrap();
    /// ```
    pub fn bind_with_enc<A: ToSocketAddrs>(
        addr: A,
        enc_key: [u8; 32],
    ) -> Result<ChannelServer<S, R>> {
        Ok(ChannelServer {
            listener: TcpListener::bind(addr)?,
            enc_key: Some(enc_key),
            _sender: Default::default(),
            _receiver: Default::default(),
        })
    }
}

impl<S, R> Deref for ChannelServer<S, R> {
    type Target = TcpListener;

    fn deref(&self) -> &Self::Target {
        &self.listener
    }
}

impl<S, R> Iterator for ChannelServer<S, R> {
    type Item = (ChannelSender<S>, ChannelReceiver<R>, SocketAddr);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self
                .listener
                .incoming()
                .next()
                .expect("TcpListener::incoming returned None");
            // `next` is Err if a client connected only partially
            if let Ok(mut sender) = next {
                // it is required for all the clients to have a proper SocketAddr
                let peer_addr = sender.peer_addr().expect("Peer has no remote address");
                // it is required that the sockets are clonable for splitting them into
                // sender/receiver
                let receiver = sender.try_clone().expect("Failed to clone the stream");
                // if the encryption key was provided do the handshake using it
                if let Some(enc_key) = &self.enc_key {
                    let key = Key::from_slice(enc_key);

                    // generate and exchange new random nonce
                    let (enc_nonce, dec_nonce) = match nonce_handshake(&mut sender) {
                        Ok(x) => x,
                        Err(e) => {
                            warn!("Nonce handshake failed with {}: {:?}", peer_addr, e);
                            continue;
                        }
                    };
                    let enc_nonce = Nonce::from_slice(&enc_nonce);
                    let mut enc = ChaCha20::new(&key, &enc_nonce);

                    let dec_nonce = Nonce::from_slice(&dec_nonce);
                    let mut dec = ChaCha20::new(&key, &dec_nonce);

                    // the last part of the handshake checks that the encryption key is correct
                    if let Err(e) = check_encryption_key(&mut sender, &mut enc, &mut dec) {
                        warn!("Magic handshake failed with {}: {:?}", peer_addr, e);
                        continue;
                    }

                    return Some((
                        ChannelSender {
                            inner: ChannelSenderInner::RemoteEnc(Arc::new(Mutex::new((
                                sender, enc,
                            )))),
                        },
                        ChannelReceiver {
                            inner: ChannelReceiverInner::RemoteEnc(RefCell::new((receiver, dec))),
                        },
                        peer_addr,
                    ));
                // if no key was provided the handshake checks that the other end also didn't use an
                // encryption key
                } else {
                    if let Err(e) = check_no_encryption(&mut sender) {
                        warn!("Magic handshake failed with {}: {:?}", peer_addr, e);
                        continue;
                    }
                    return Some((
                        ChannelSender {
                            inner: ChannelSenderInner::Remote(Arc::new(Mutex::new(sender))),
                        },
                        ChannelReceiver {
                            inner: ChannelReceiverInner::Remote(RefCell::new(receiver)),
                        },
                        peer_addr,
                    ));
                }
            }
        }
    }
}

/// Connect to a remote channel.
///
/// All the remote channels are full-duplex, therefore this function returns a channel for sending
/// the message and a channel from where receive them.
///
/// This function will no enable message encryption.
///
/// ```
/// # use ductile::{ChannelServer, connect_channel};
/// let port = 18455; // let's hope we can bind this port!
/// let mut server = ChannelServer::<(), _>::bind(("127.0.0.1", port)).unwrap();
///
/// let client_thread = std::thread::spawn(move || {
///     let (sender, receiver) = connect_channel::<_, _, ()>(("127.0.0.1", port)).unwrap();
///
///     sender.send(vec![1, 2, 3, 4]).unwrap();
/// });
///
/// let (sender, receiver, _addr) = server.next().unwrap();
/// let data: Vec<i32> = receiver.recv().unwrap();
/// assert_eq!(data, vec![1, 2, 3, 4]);
/// # client_thread.join().unwrap();
/// ```

pub fn connect_channel<A: ToSocketAddrs, S, R>(
    addr: A,
) -> Result<(ChannelSender<S>, ChannelReceiver<R>)> {
    let mut stream = TcpStream::connect(addr)?;
    let stream2 = stream.try_clone()?;
    check_no_encryption(&mut stream)?;
    Ok((
        ChannelSender {
            inner: ChannelSenderInner::Remote(Arc::new(Mutex::new(stream))),
        },
        ChannelReceiver {
            inner: ChannelReceiverInner::Remote(RefCell::new(stream2)),
        },
    ))
}

/// Connect to a remote channel encrypting the connection.
///
/// All the remote channels are full-duplex, therefore this function returns a channel for sending
/// the message and a channel from where receive them.
///
/// ```
/// # use ductile::{ChannelServer, connect_channel, connect_channel_with_enc};
/// let port = 18456; // let's hope we can bind this port!
/// let key = [42; 32];
/// let mut server = ChannelServer::<(), _>::bind_with_enc(("127.0.0.1", port), key).unwrap();
///
/// let client_thread = std::thread::spawn(move || {
///     let (sender, receiver) = connect_channel_with_enc::<_, _, ()>(("127.0.0.1", port), &key).unwrap();
///
///     sender.send(vec![1, 2, 3, 4]).unwrap();
/// });
///
/// let (sender, receiver, _addr) = server.next().unwrap();
/// let data: Vec<i32> = receiver.recv().unwrap();
/// assert_eq!(data, vec![1, 2, 3, 4]);
/// # client_thread.join().unwrap();
/// ```
pub fn connect_channel_with_enc<A: ToSocketAddrs, S, R>(
    addr: A,
    enc_key: &[u8; 32],
) -> Result<(ChannelSender<S>, ChannelReceiver<R>)> {
    let mut stream = TcpStream::connect(addr)?;
    let stream2 = stream.try_clone()?;

    let (enc_nonce, dec_nonce) = nonce_handshake(&mut stream)?;
    let key = Key::from_slice(enc_key);
    let mut enc = ChaCha20::new(&key, &Nonce::from_slice(&enc_nonce));
    let mut dec = ChaCha20::new(&key, &Nonce::from_slice(&dec_nonce));

    check_encryption_key(&mut stream, &mut enc, &mut dec)?;

    Ok((
        ChannelSender {
            inner: ChannelSenderInner::RemoteEnc(Arc::new(Mutex::new((stream, enc)))),
        },
        ChannelReceiver {
            inner: ChannelReceiverInner::RemoteEnc(RefCell::new((stream2, dec))),
        },
    ))
}

/// Send the encryption nonce and receive the decryption nonce using the provided socket.
fn nonce_handshake(s: &mut TcpStream) -> Result<([u8; 12], [u8; 12])> {
    let mut enc_nonce = [0u8; 12];
    OsRng.fill_bytes(&mut enc_nonce);
    s.write_all(&enc_nonce)?;
    s.flush()?;

    let mut dec_nonce = [0u8; 12];
    s.read_exact(&mut dec_nonce)?;

    Ok((enc_nonce, dec_nonce))
}

/// Check that the encryption key is the same both ends.
fn check_encryption_key(
    stream: &mut TcpStream,
    enc: &mut ChaCha20,
    dec: &mut ChaCha20,
) -> Result<()> {
    let mut magic = MAGIC.to_le_bytes();
    enc.apply_keystream(&mut magic);
    stream.write_all(&magic)?;
    stream.flush()?;

    stream.read_exact(&mut magic)?;
    dec.apply_keystream(&mut magic);
    let magic = u32::from_le_bytes(magic);
    if magic != MAGIC {
        bail!("Wrong encryption key");
    }
    Ok(())
}

/// Check that no encryption is used by the other end.
fn check_no_encryption(stream: &mut TcpStream) -> Result<()> {
    let key = b"task-maker's the best thing ever";
    let nonce = b"task-maker!!";
    let mut enc = ChaCha20::new(Key::from_slice(key), Nonce::from_slice(nonce));
    let mut dec = ChaCha20::new(Key::from_slice(key), Nonce::from_slice(nonce));
    check_encryption_key(stream, &mut enc, &mut dec)
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    #[test]
    fn test_remote_channels_enc_wrong_key() {
        let port = rand::thread_rng().gen_range(10000u16, 20000u16);
        let enc_key = [42u8; 32];
        let mut server: ChannelServer<(), ()> =
            ChannelServer::bind_with_enc(("127.0.0.1", port), enc_key).unwrap();

        let client_thread = std::thread::spawn(move || {
            let wrong_enc_key = [69u8; 32];
            assert!(
                connect_channel_with_enc::<_, (), ()>(("127.0.0.1", port), &wrong_enc_key).is_err()
            );

            // the call to .next() below blocks until a client connects successfully
            connect_channel_with_enc::<_, (), ()>(("127.0.0.1", port), &enc_key).unwrap();
        });

        server.next().unwrap();
        client_thread.join().unwrap();
    }

    #[test]
    fn test_remote_channels_enc_no_key() {
        let port = rand::thread_rng().gen_range(10000u16, 20000u16);
        let enc_key = [42u8; 32];
        let mut server: ChannelServer<(), ()> =
            ChannelServer::bind_with_enc(("127.0.0.1", port), enc_key).unwrap();

        let client_thread = std::thread::spawn(move || {
            assert!(connect_channel::<_, (), ()>(("127.0.0.1", port)).is_err());

            // the call to .next() below blocks until a client connects successfully
            connect_channel_with_enc::<_, (), ()>(("127.0.0.1", port), &enc_key).unwrap();
        });

        server.next().unwrap();
        client_thread.join().unwrap();
    }

    #[test]
    fn test_remote_channels_receiver_stops() {
        let port = rand::thread_rng().gen_range(10000u16, 20000u16);
        let mut server: ChannelServer<(), ()> = ChannelServer::bind(("127.0.0.1", port)).unwrap();

        let client_thread = std::thread::spawn(move || {
            let (sender, _) = connect_channel::<_, (), ()>(("127.0.0.1", port)).unwrap();
            sender.send(()).unwrap();
        });

        let (_, receiver, _) = server.next().unwrap();
        client_thread.join().unwrap();
        receiver.recv().unwrap();
        // all the senders have been dropped and there are no more messages
        assert!(receiver.recv().is_err());
    }
}
