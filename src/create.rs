//! Functions to spawn a [`neovim`](crate::neovim::Neovim) session.
//!
//! This implements various possibilities to connect to neovim, including
//! spawning an own child process. Available capabilities might depend on your
//! OS.
//!
//! Right now, this depends on the `use_tokio` feature, since it uses tokio
//! under the hood.
use std::{
  io::{self, Error, ErrorKind},
  path::Path,
  process::Stdio,
};

#[cfg(unix)]
use tokio::net::UnixStream;

use tokio::{
  io::{split, stdin, stdout, Stdout, WriteHalf},
  net::{TcpStream, ToSocketAddrs},
  process::{Child, ChildStdin, Command},
  spawn,
  task::JoinHandle,
};

use crate::{
  compat::tokio::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt},
  error::LoopError,
  neovim::Neovim,
  Handler,
};

/// Connect to a neovim instance via tcp
pub async fn new_tcp<A, H>(
  addr: A,
  handler: H,
) -> io::Result<(
  Neovim<Compat<WriteHalf<TcpStream>>>,
  JoinHandle<Result<(), Box<LoopError>>>,
)>
where
  H: Handler<Writer = Compat<WriteHalf<TcpStream>>>,
  A: ToSocketAddrs,
{
  let stream = TcpStream::connect(addr).await?;
  let (reader, writer) = split(stream);
  let (neovim, io) = Neovim::<Compat<WriteHalf<TcpStream>>>::new(
    reader.compat_read(),
    writer.compat_write(),
    handler,
  );
  let io_handle = spawn(io);

  Ok((neovim, io_handle))
}

#[cfg(unix)]
/// Connect to a neovim instance via unix socket
pub async fn new_unix_socket<H, P: AsRef<Path> + Clone>(
  path: P,
  handler: H,
) -> io::Result<(
  Neovim<Compat<WriteHalf<UnixStream>>>,
  JoinHandle<Result<(), Box<LoopError>>>,
)>
where
  H: Handler<Writer = Compat<WriteHalf<UnixStream>>> + Send + 'static,
{
  let stream = UnixStream::connect(path).await?;
  let (reader, writer) = split(stream);
  let (neovim, io) = Neovim::<Compat<WriteHalf<UnixStream>>>::new(
    reader.compat_read(),
    writer.compat_write(),
    handler,
  );
  let io_handle = spawn(io);

  Ok((neovim, io_handle))
}

/// Connect to a neovim instance by spawning a new one
pub async fn new_child<H>(
  handler: H,
) -> io::Result<(
  Neovim<Compat<ChildStdin>>,
  JoinHandle<Result<(), Box<LoopError>>>,
  Child,
)>
where
  H: Handler<Writer = Compat<ChildStdin>> + Send + 'static,
{
  if cfg!(target_os = "windows") {
    new_child_path("nvim.exe", handler).await
  } else {
    new_child_path("nvim", handler).await
  }
}

/// Connect to a neovim instance by spawning a new one
pub async fn new_child_path<H, S: AsRef<Path>>(
  program: S,
  handler: H,
) -> io::Result<(
  Neovim<Compat<ChildStdin>>,
  JoinHandle<Result<(), Box<LoopError>>>,
  Child,
)>
where
  H: Handler<Writer = Compat<ChildStdin>> + Send + 'static,
{
  new_child_cmd(Command::new(program.as_ref()).arg("--embed"), handler).await
}

/// Connect to a neovim instance by spawning a new one
///
/// stdin/stdout will be rewritten to `Stdio::piped()`
pub async fn new_child_cmd<H>(
  cmd: &mut Command,
  handler: H,
) -> io::Result<(
  Neovim<Compat<ChildStdin>>,
  JoinHandle<Result<(), Box<LoopError>>>,
  Child,
)>
where
  H: Handler<Writer = Compat<ChildStdin>> + Send + 'static,
{
  let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
  let stdout = child
    .stdout
    .take()
    .ok_or_else(|| Error::new(ErrorKind::Other, "Can't open stdout"))?
    .compat_read();
  let stdin = child
    .stdin
    .take()
    .ok_or_else(|| Error::new(ErrorKind::Other, "Can't open stdin"))?
    .compat_write();

  let (neovim, io) = Neovim::<Compat<ChildStdin>>::new(stdout, stdin, handler);
  let io_handle = spawn(io);

  Ok((neovim, io_handle, child))
}

/// Connect to the neovim instance that spawned this process over stdin/stdout
pub async fn new_parent<H>(
  handler: H,
) -> (
  Neovim<Compat<Stdout>>,
  JoinHandle<Result<(), Box<LoopError>>>,
)
where
  H: Handler<Writer = Compat<Stdout>>,
{
  let (neovim, io) = Neovim::<Compat<Stdout>>::new(
    stdin().compat_read(),
    stdout().compat_write(),
    handler,
  );
  let io_handle = spawn(io);

  (neovim, io_handle)
}
