use std::{
    error::Error,
    io::{Read, Write},
    env,
    net::{SocketAddr, TcpStream},
};

use utils::Server;

fn handler(mut conn: TcpStream) -> Result<(), Box<dyn Error>> {
    let mut buf = Vec::with_capacity(1024);
    conn.read_to_end(&mut buf)?;
    conn.write_all(&buf)?;
    Ok(())
}

fn main() {
    let port = env::var("PORT").unwrap().parse().unwrap();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let server = Server::new(handler).unwrap();
    server.listen(addr).unwrap();
}
