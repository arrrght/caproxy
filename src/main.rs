extern crate pretty_env_logger;
#[macro_use] extern crate log;

use futures::future;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Method, Body, Client, Request, Response, Server};
use std::collections::HashMap;
use rand::Rng;
use std::time::Instant;
use std::sync::{Arc, Mutex};
use bytes::{ Buf, Bytes, IntoBuf };


#[derive(Debug)]
struct Prox {
    url: String,
    date: std::time::Instant
}
impl Prox {
    fn new(url: String) -> Prox {
        Prox { url: url, date: Instant::now() }
    }
}

fn get_random_proxy() -> String {
    let mut rng = rand::thread_rng();
    let urls = ["http://cap.avtocod.ru", "http://cap2.avtocod.ru"];
    urls[rng.gen_range(0,urls.len())].to_string()
}

fn change_req(proxy_now: String, r: Arc<Mutex<HashMap<usize, Prox>>>, mut req: Request<Body>) -> Request<Body>{
    info!("REQ1 {:?}", req);

    let lock = match r.lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner()
    };

    let proxy_insert = if req.uri().path() == "/res.php" {
        let s1: String = req.uri().query().unwrap().to_string().split("&").filter(|x| &x[0..3] == "id=").collect();
        let s2 = match &s1[3..].parse::<usize>() {
            Ok(r) => r.to_owned(),
            Err(_e) => 0 as usize
        };
        let url4get = match lock.get(&s2) {
            None => &proxy_now,
            Some(o) => &o.url
        };
        url4get
    }else{ 
        &proxy_now
    };

    info!("GOT NOW proxy:{}, hash:{:?}", proxy_insert, lock);

    let uri_string = format!("{}{}", proxy_insert, req.uri().path_and_query().map(|x| x.as_str()).unwrap_or(""));
    let uri = uri_string.parse().expect("here2");
    *req.uri_mut() = uri;
    req.headers_mut().remove("host");
    req
}

fn main() {
    pretty_env_logger::init();

    let proxies: HashMap<usize, Prox> = HashMap::new();
    let r = Arc::new(Mutex::new(proxies));

    let in_addr = ([127, 0, 0, 1], 3001).into();
    let client_main = Client::new();

    let proxy = move || {
        let client = client_main.clone();
        let inner = Arc::clone(&r);

        service_fn(move |req| {
            let proxy_now = get_random_proxy();
            let proxy_now2 = proxy_now.clone();
            let inner2 = Arc::clone(&inner);
            let inner3 = Arc::clone(&inner);
            let req = change_req(proxy_now, inner2, req);
            debug!("REDIR_REQ: {} / {}", req.method(), req.uri());
            client
                .request(req)
                .and_then(move |res| {
                    debug!("res: {:?}", res);
                    res.into_body().concat2()
                })
                .and_then(move |body| {
                    debug!("body: {:?}", body);
                    let body_plain = std::str::from_utf8(&body).map(str::to_owned).map_err(|_x| ());
                    match body_plain {
                        Ok(ans) => {
                            if Bytes::from(&ans[0..3]) == Bytes::from(&b"OK|"[..]) {
                                // yes, it's OK answer, save it
                                let ok_answer_str = &String::from_utf8(Bytes::from(&ans[3..]).into_buf().collect()).unwrap();
                                match ok_answer_str.parse::<usize>() {
                                    Ok(r) => {
                                        info!("OK ANS: {:?}, ", r);
                                        let mut lock = match inner3.lock() {
                                            Ok(guard) => guard,
                                            Err(poison) => poison.into_inner()
                                        };
                                        let prox = Prox::new(proxy_now2);
                                        lock.insert(r, prox);
                                    },
                                    Err(_e) => debug!("not yet")
                                }
                            }
                            future::ok(Response::new(Body::from(body)))
                        },
                        Err(_e) => {
                            future::ok(Response::new(Body::from(body)))
                        }
                    }
                })
        })
    };

    let server = Server::bind(&in_addr).serve(proxy).map_err(|e| eprintln!("server error: {}", e));
    println!("Listening on http://{}", in_addr);
    hyper::rt::run(server);
}
