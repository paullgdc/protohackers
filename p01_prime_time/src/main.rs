use std::{
    borrow::Cow,
    env,
    error::Error,
    io::{self, BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
};

use utils::{
    json::{self, Value},
    Server,
};

fn write_error(s: &mut TcpStream) -> io::Result<()> {
    s.write_all(b"{\"error\": \"malformed request\"}")?;
    Ok(())
}

fn is_prime(n: i64) -> bool {
    if n < 2 {
        return false;
    }
    let n = n as u64;
    for i in 2..=((n as f64).sqrt() as u64) {
        dbg!(i);
        if n.rem_euclid(i) == 0 {
            return false;
        }
    }
    true
}

fn handle(mut s: TcpStream) -> Result<(), Box<dyn Error>> {
    let mut reader = BufReader::new(s.try_clone()?);
    let mut req_buf = Vec::new();
    let mut res_buf = Vec::new();
    loop {
        req_buf.clear();
        res_buf.clear();
        match reader.read_until(b'\n', &mut req_buf) {
            Ok(read) if read == 0 => {
                write_error(&mut s)?;
                break;
            }
            res => res,
        }?;
        let req = match json::parse_json(&req_buf) {
            Ok(v) => v,
            Err(e) => {
                utils::log_err!("Failed parsing json {:?}", e);
                write_error(&mut s)?;
                break;
            }
        };
        let arg = match req
            .object()
            .and_then(|o| Some((o.get("method")?.string()?, o.get("prime")?.int()?)))
        {
            Some((method, arg)) if method == "isPrime" => *arg,
            _ => {
                utils::log_info!("Non conforming payloads");
                write_error(&mut s)?;
                break;
            }
        };
        json::serialize_json(
            &Value::Object(
                [
                    (
                        Cow::Borrowed("method"),
                        Value::String(Cow::Borrowed("isPrime")),
                    ),
                    (Cow::Borrowed("prime"), Value::Bool(is_prime(arg))),
                ]
                .into_iter()
                .collect(),
            ),
            &mut res_buf,
        );
        res_buf.push(b'\n');
        s.write_all(&res_buf)?;
    }
    Ok(())
}

fn main() {
    let port = env::var("PORT").unwrap().parse().unwrap();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let server = Server::new(handle).unwrap();
    server.listen(addr).unwrap();
}
