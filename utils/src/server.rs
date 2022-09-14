use std::{
    error::Error,
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    thread,
};

use crate::{log_err, log_info};

pub struct Server {
    conn_handler: Box<dyn Fn(TcpStream) -> Result<(), Box<dyn Error>> + Sync>,
}

impl Server {
    pub fn new<F>(handler: F) -> io::Result<Self>
    where
        F: Fn(TcpStream) -> Result<(), Box<dyn Error>> + Sync + 'static,
    {
        Ok(Self {
            conn_handler: Box::new(handler),
        })
    }

    pub fn listen(&self, addr: SocketAddr) -> io::Result<()> {
        thread::scope(|s| {
            let listener = TcpListener::bind(addr)?;
            log_info!("Listening on {}", addr);
            for incoming in listener.incoming() {
                s.spawn(|| {
                    let conn = match incoming {
                        Ok(conn) => conn,
                        Err(e) => return log_err!("accepting connection: {}", e),
                    };
                    let peer = match conn.peer_addr() {
                        Ok(peer) => peer,
                        Err(e) => return log_err!("getting peer address: {}", e),
                    };
                    eprintln!("Handling connection from {peer}");
                    match std::panic::catch_unwind(|| (self.conn_handler)(conn)) {
                        Ok(Ok(())) => {
                            log_info!("Connection from {} closed", peer)
                        }
                        Ok(Err(e)) => log_err!("handling connection from {}: {}", peer, e),
                        Err(e) => {
                            log_err!("handling for connection from {} panicked: {}", peer, e);
                            return;
                        }
                    };
                });
            }
            Ok(())
        })
    }
}
