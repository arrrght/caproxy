use futures::future;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Method, Body, Client, Request, Response, Server, StatusCode};
use std::io::{self, Write};
use std::collections::HashMap;
use rand::Rng;
use std::time::Instant;
use std::sync::{Arc, Mutex};
use bytes::{ Buf, Bytes, IntoBuf };
use url::{Url, ParseError};

struct Prox {
    url: String,
    date: std::time::Instant
}
impl Prox {
    fn new(url: String) -> Prox {
        Prox { url: url, date: std::time::Instant::now() }
    }
}

fn change_req(label: usize, mut r: Arc<Mutex<HashMap<usize, String>>>, mut req: Request<Body>) -> Request<Body>{
    println!("REQ1 {:?}", req);
    let mut rng = rand::thread_rng();
    let urls = ["http://cap.avtocod.ru", "http://cap2.avtocod.ru"];
    let rnd_url = urls[rng.gen_range(0,urls.len())].to_string();

    println!("REQM {:?}", req.method());

    r.lock().expect("here1");
    let mut lock = match r.lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner()
    };
    let now_proxy = if req.method() == Method::GET {
        let s1: String = req.uri().query().unwrap().to_string().split("&").filter(|x| &x[0..3] == "id=").collect();
        let s2 = match &s1[3..].parse::<usize>() {
            Ok(r) => r.to_owned(),
            Err(_e) => 0 as usize
        };
        let url4get = lock.get(&s2).unwrap().to_owned();
        println!("GOT GET {}, {:?} {:?}", s2, label, url4get);
        url4get
    }else{ 
        match lock.contains_key(&label) {
            true => lock.get(&label).unwrap().to_owned(),
            false => { lock.insert(label, rnd_url.to_string()); rnd_url }
        }
    };

    println!("GOT THIS: {}:: {:?}", now_proxy, lock);

    let uri_string = format!("{}{}", now_proxy, req.uri().path_and_query().map(|x| x.as_str()).unwrap_or(""));
    let uri = uri_string.parse().expect("here2");
    *req.uri_mut() = uri;
    req.headers_mut().remove("host");
    //req.headers_mut().insert("label", label.to_string().parse().unwrap());
    req
}

fn main() {
    pretty_env_logger::init();

    let proxies: HashMap<usize, String> = HashMap::new();
    let r = Arc::new(Mutex::new(proxies));
    //let r = Arc::new(String::new());

    let in_addr = ([127, 0, 0, 1], 3001).into();
    let client_main = Client::new();

    let proxy = move || {
        let client = client_main.clone();
        let inner = Arc::clone(&r);

        service_fn(move |req| {
            let mut rng = rand::thread_rng();
            let path_var: usize = rng.gen();
            let inner2 = Arc::clone(&inner);
            let req = change_req(path_var, inner2, req);
            let inner3 = Arc::clone(&inner);
            println!("PATH_REQ: {}", path_var);
            client
                .request(req)
                .and_then(move |res| {
                    println!("PATH_RES: {}", path_var);
                    println!("res: {:?}", res);
                    res.into_body().concat2()
                })
                .and_then(move |body| {
                    println!("body: {:?}", body);
                    let body_plain = std::str::from_utf8(&body).map(str::to_owned).map_err(|_x| ());
                    match body_plain {
                        Ok(ans) => {
                            println!("PATH_BODY: {:?}", ans);
                            if Bytes::from(&ans[0..3]) == Bytes::from(&b"OK|"[..]) {
                                // yes, it's OK answer, save it
                                let ok_answer_str = &String::from_utf8(Bytes::from(&ans[3..]).into_buf().collect()).unwrap();
                                let ok_answer = match ok_answer_str.parse::<usize>() {
                                    Ok(r) => r.to_owned(),
                                    Err(_e) => 0 as usize
                                };
                                println!("OK ANS: {:?}", ok_answer);

                                let mut lock = match inner3.lock() {
                                    Ok(guard) => guard,
                                    Err(poison) => poison.into_inner()
                                };
                                match lock.contains_key(&path_var) {
                                    true => {
                                        let proxy = lock.get(&path_var).unwrap().to_owned();
                                        lock.insert(ok_answer, proxy);
                                    },
                                    false => () //panic!("panicked as a bitch")
                                };
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
