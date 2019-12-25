use std::{
  future::Future,
  io::{self, Error, ErrorKind},
  path::Path,
  process::Stdio,
};

use crate::{
  callerror::LoopError,
  runtime::{ChildStdin, Command, Stdout, TcpStream, Child},
  Handler, neovim::Neovim,
};

#[cfg(unix)]
use crate::runtime::{stdin, stdout, UnixStream};

/// Connect to nvim instance via tcp
pub async fn new_tcp<H>(
  host: &str,
  port: u16,
  handler: H,
) -> io::Result<(
  Neovim<TcpStream>,
  impl Future<Output = Result<(), Box<LoopError>>>,
)>
where
  H: Handler<Writer = TcpStream> + Send + 'static,
{
  let stream = TcpStream::connect((host, port)).await?;
  let read = TcpStream::connect((host, port)).await?;
  //let read = stream.try_clone()?;
  let (requester, fut) = Neovim::<TcpStream>::new(stream, read, handler);

  Ok((requester, fut))
}

#[cfg(unix)]
/// Connect to nvim instance via unix socket
pub async fn new_unix_socket<H, P: AsRef<Path> + Clone>(
  path: P,
  handler: H,
) -> io::Result<(
  Neovim<UnixStream>,
  impl Future<Output = Result<(), Box<LoopError>>>,
)>
where
  H: Handler<Writer = UnixStream> + Send + 'static,
{
  let stream = UnixStream::connect(path.clone()).await?;
  let read = UnixStream::connect(path).await?;
  //let read = stream.try_clone()?;

  let (requester, fut) = Neovim::<UnixStream>::new(stream, read, handler);

  Ok((requester, fut))
}

/// Connect to a Neovim instance by spawning a new one.
pub async fn new_child<H>(
  handler: H,
) -> io::Result<(
  Neovim<ChildStdin>,
  impl Future<Output = Result<(), Box<LoopError>>>,
  Child,
)>
where
  H: Handler<Writer = ChildStdin> + Send + 'static,
{
  if cfg!(target_os = "windows") {
    new_child_path("nvim.exe", handler).await
  } else {
    new_child_path("nvim", handler).await
  }
}

/// Connect to a Neovim instance by spawning a new one
pub async fn new_child_path<H, S: AsRef<Path>>(
  program: S,
  handler: H,
) -> io::Result<(
  Neovim<ChildStdin>,
  impl Future<Output = Result<(), Box<LoopError>>>,
  Child
)>
where
  H: Handler<Writer = ChildStdin> + Send + 'static,
{
  new_child_cmd(Command::new(program.as_ref()).arg("--embed"), handler).await
}

/// Connect to a Neovim instance by spawning a new one
///
/// stdin/stdout settings will be rewrited to `Stdio::piped()`
pub async fn new_child_cmd<H>(
  cmd: &mut Command,
  handler: H,
) -> io::Result<(
  Neovim<ChildStdin>,
  impl Future<Output = Result<(), Box<LoopError>>>,
  Child,
)>
where
  H: Handler<Writer = ChildStdin> + Send + 'static,
{
  let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
  let stdout = child
    .stdout()
    .take()
    .ok_or_else(|| Error::new(ErrorKind::Other, "Can't open stdout"))?;
  let stdin = child
    .stdin()
    .take()
    .ok_or_else(|| Error::new(ErrorKind::Other, "Can't open stdin"))?;

  let (requester, fut) = Neovim::<ChildStdin>::new(stdout, stdin, handler);

  Ok((requester, fut, child))
}

/// Connect to a Neovim instance that spawned this process over stdin/stdout.
pub fn new_parent<H>(
  handler: H,
) -> io::Result<(
  Neovim<Stdout>,
  impl Future<Output = Result<(), Box<LoopError>>>,
)>
where
  H: Handler<Writer = Stdout> + Send + 'static,
{
  let (requester, fut) = Neovim::<Stdout>::new(stdin(), stdout(), handler);

  Ok((requester, fut))
}
