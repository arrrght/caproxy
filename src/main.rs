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

#[derive(Debug)]
struct Proxies {
    urls: Vec<String>,
    list: HashMap<usize, Prox>
}

impl Proxies {
    fn get_proxy(&self) -> String {
        let mut rng = rand::thread_rng();
        self.urls[rng.gen_range(0, self.urls.len())].to_owned()
    }
    fn add_proxy(&mut self, url: &str) {
        self.urls.push(url.to_owned());
    }
    fn new() -> Proxies { 
        Proxies {
            urls: Vec::new(),
            list: HashMap::new()
        }
    }
    fn get(&self, id: usize) -> Option<String> {
        match self.list.get(&id) {
            Some(o) => Some(o.url.clone()),
            None => None
        }
    }
    fn set(&mut self, id: usize, url: String) {
        let prox = Prox { url: url, date: Instant::now() };
        self.list.insert(id, prox);

        // cleanup
        let now = Instant::now();
        let r2del: Vec<usize> = self.list.iter().filter(|&(_,v)| now.duration_since(v.date).as_secs() > 60).map(|(k,_)| k.to_owned()).collect();
        let _consumed: Vec<_> = r2del.iter().map(|i| self.list.remove(i)).collect();
        info!("r2d: {:?}", r2del);
    }
}

fn change_req(proxy_now: String, r: Arc<Mutex<Proxies>>, mut req: Request<Body>) -> Request<Body>{
    info!("REQ1 {:?}", req);

    let lock = match r.lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner()
    };

    let proxy_insert = if req.uri().path() == "/res.php" {
        let s1: String = req.uri().query().unwrap().to_string().split("&").filter(|x| &x[0..3] == "id=").collect();
        match s1[3..].parse::<usize>() {
            Ok(r) => {
                match lock.get(r) {
                    Some(o) => o,
                    None => proxy_now
                }
            },
            Err(_e) => proxy_now
        }
    }else{ 
        proxy_now
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

    let mut proxies = Proxies::new();
    proxies.add_proxy("http://cap.avtocod.ru");
    proxies.add_proxy("http://cap2.avtocod.ru");
    let r = Arc::new(Mutex::new(proxies));

    let in_addr = ([127, 0, 0, 1], 3001).into();
    let client_main = Client::new();

    let proxy = move || {
        let client = client_main.clone();
        let inner = Arc::clone(&r);
        let inner2 = Arc::clone(&r);

        service_fn(move |req| {
            let proxy_now = {
                inner2.lock().unwrap().get_proxy()
            };
            let proxy_now2 = proxy_now.clone();
            let inner3 = Arc::clone(&inner);
            let inner4 = Arc::clone(&inner);
            let req = change_req(proxy_now, inner3, req);
            debug!("REDIR_REQ: {} / {}", req.method(), req.uri());
            client
                .request(req)
                .and_then(move |res| {
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
                                        let mut lock = match inner4.lock() {
                                            Ok(guard) => guard,
                                            Err(poison) => poison.into_inner()
                                        };
                                        //let prox = Prox::new(proxy_now2);
                                        //lock.insert(r, prox);
                                        lock.set(r, proxy_now2);
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
