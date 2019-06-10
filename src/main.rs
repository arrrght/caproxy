#[allow(unused_imports)]
use log::{debug, info, warn};

use bytes::{Buf, Bytes, IntoBuf};
use futures::future;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Client, Request, Response, Server};
use lazy_static::lazy_static;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[macro_use]
extern crate prometheus;
use prometheus::{Encoder, IntGaugeVec, TextEncoder};

mod errors;
use errors::CheckersErr;
mod structs;
use structs::{Proxies, Stat};

lazy_static! {
    static ref RETRY_CNT: IntGaugeVec = register_int_gauge_vec!("cap_retry_counter", "Retry counter", &["handler"]).unwrap();
    static ref ACCESS_TIME: IntGaugeVec = register_int_gauge_vec!("cap_access_time", "Access time to CapMonster", &["handler"]).unwrap();
    static ref IS_ALIVE: IntGaugeVec = register_int_gauge_vec!("cap_alive", "CapMonster is alive", &["handler"]).unwrap();
    static ref CNT: IntGaugeVec = register_int_gauge_vec!("cap_count", "Counter", &["handler"]).unwrap();
    static ref WEIGHT: IntGaugeVec = register_int_gauge_vec!("cap_weight", "Weights", &["handler"]).unwrap();
}

fn change_req(proxy_now: String, r: Arc<Mutex<Proxies>>, mut req: Request<Body>) -> (Option<Stat>, Request<Body>) {
    info!("REQ | RAW {:?}", req);

    let mut lock = match r.lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner(),
    };

    let proxy_insert = if req.uri().path() == "/res.php" {
        let s1: String = req.uri().query().unwrap().to_string().split("&").filter(|x| &x[0..3] == "id=").collect();
        match s1[3..].parse::<usize>() {
            Ok(r) => match lock.get(r) {
                Some(o) => o,
                None => proxy_now,
            },
            Err(_e) => proxy_now,
        }
    } else {
        proxy_now
    };

    let stat: Option<Stat> = if req.uri().path() == "/in.php" {
        Some(Stat {
            url: proxy_insert.clone(),
            dt_start: Instant::now(),
        })
    } else {
        None
    };

    info!("REQ | AFTER GOT NOW proxy:{}, hash-len:{:?}", proxy_insert, lock.urls.len());

    let uri_string = format!("{}{}", proxy_insert, req.uri().path_and_query().map(|x| x.as_str()).unwrap_or(""));
    let uri = uri_string.parse().expect("here2");
    *req.uri_mut() = uri;
    req.headers_mut().remove("host");
    (stat, req)
}

fn run_checkers(wait: u64, proxy_now: String) -> Result<(usize, u32), CheckersErr> {
    let time_start = Instant::now();
    let client = reqwest::Client::new().post(&format!("{}/in.php", proxy_now));
    let file_part = reqwest::multipart::Part::bytes(&include_bytes!("generate.jpg")[..]).file_name("generate.jpg").mime_str("image/jpeg")?;
    let form = reqwest::multipart::Form::new().text("method", "post").part("file", file_part);

    let mut response = client.multipart(form).send()?;
    let mut response_body = String::new();
    response.read_to_string(&mut response_body)?;
    let ans: Vec<&str> = response_body.split("|").collect();
    if ans.len() < 2 { return Err(CheckersErr::Other("Isn't answer".to_owned())) }
    let ans_int: usize = ans[1].to_string().parse()?;

    let mut ret: Option<Result<(usize, u32), CheckersErr>> = None;
    let mut retry_cnt: usize = 0;
    while ret.is_none() || retry_cnt > 9 {
        retry_cnt += 1;
        let check_api = format!("{}/res.php?action=get&id={}", proxy_now, ans_int);
        let req_get = reqwest::get(&check_api)?.text();
        ret = match req_get?.as_ref() {
            "CAPCHA_NOT_READY" => None,
            "OK|xab35" => {
                let time_elapsed = time_start.elapsed().subsec_millis();
                Some(Ok((retry_cnt, time_elapsed)))
            }
            _ => None,
        };
        std::thread::sleep(std::time::Duration::from_millis(wait));
    }
    match ret {
        Some(x) => x,
        None => Err(CheckersErr::Other("Fuck this, i'm None".to_owned())),
    }
}

fn main() {
    pretty_env_logger::init();
    let mut proxies_env: Vec<(String, isize)> = std::env::var("CAPS")
        .expect("CAPS environment not set")
        .split(",")
        .map(|x| {
            let a: Vec<&str> = x.split("=").collect();
            let name = a[1..].join("=");
            let weight = a[0].parse::<isize>().expect("Can not parse weight on CAPS environment");
            WEIGHT.with_label_values(&[&name]).set(weight.abs() as i64);
            (name, weight)
        })
        .collect();

    let proxies = Proxies::new(proxies_env.clone());

    let cap_check_period = std::env::var("CAP_CHECK_PERIOD") .unwrap_or("5000".to_owned()) .parse::<u64>() .unwrap_or(5000);
    let cap_check_wait = std::env::var("CAP_CHECK_WAIT").unwrap_or("200".to_owned()).parse::<u64>().unwrap_or(200);
    let in_addr: std::net::SocketAddr = std::env::var("CAP_LISTEN").unwrap_or("0.0.0.0:8080".to_owned()).parse().expect("can't parse listen addr");

    info!("== RUN with ==");
    info!("CAP_CHECK_PERIOD : {:?} msec", cap_check_period);
    info!("CAP_CHECK_WAIT   : {:?} msec", cap_check_wait);
    info!("CAPS {:?}", proxies);
    info!("==============");

    let r = Arc::new(Mutex::new(proxies));
    let rr = Arc::clone(&r);

    let client_main = Client::new();

    let proxy = move || {
        let client = client_main.clone();
        let inner = Arc::clone(&r);
        let inner2 = Arc::clone(&r);

        service_fn(move |req| {
            match req.uri().path() {
                "/metrics" => {
                    let encoder = TextEncoder::new();
                    let metric_families = prometheus::gather();
                    let mut buf = Vec::<u8>::new();
                    encoder.encode(&metric_families, &mut buf).unwrap();
                    future::Either::B(future::ok(Response::new(Body::from(buf))))
                }

                _ => {
                    let proxy_now = { inner2.lock().unwrap().get_proxy() };
                    let proxy_now2 = proxy_now.clone();
                    let proxy_now3 = proxy_now.clone();
                    let inner3 = Arc::clone(&inner);
                    let inner4 = Arc::clone(&inner);
                    let (stat, req) = change_req(proxy_now, inner3, req);
                    debug!("REQ | {:?} -> {} / {}", stat, req.method(), req.uri());
                    CNT.with_label_values(&[&proxy_now3]).inc();
                    future::Either::A(client.request(req).and_then(move |res| {
                        // cut error here
                        res.into_body().concat2()
                    }).and_then(move |body| {
                        debug!("RSP | body: {:?}", body);
                        let body_plain = std::str::from_utf8(&body).map(str::to_owned).map_err(|_x| ());
                        match body_plain {
                            Ok(ans) => {
                                if ans.len() < 16 && ans.len() > 5 && Bytes::from(&ans[0..3]) == Bytes::from(&b"OK|"[..]) {
                                    let ok_answer_str = &String::from_utf8(Bytes::from(&ans[3..]).into_buf().collect()).unwrap();
                                    info!("RSP | ANS {}", &String::from_utf8(Bytes::from(&ans[..]).into_buf().collect()).unwrap());
                                    // yes, it's OK answer, save it
                                    match ok_answer_str.parse::<usize>() {
                                        Ok(r) => {
                                            let mut lock = match inner4.lock() {
                                                Ok(guard) => guard,
                                                Err(poison) => poison.into_inner(),
                                            };
                                            lock.set(r, proxy_now2);
                                        }
                                        Err(_e) => {}
                                    }
                                }
                                future::ok(Response::new(Body::from(body)))
                            }
                            Err(_e) => future::ok(Response::new(Body::from(body))),
                        }
                    }))
                }
            }
        })
    };

    // Run checkers
    while let Some(proxy_now) = proxies_env.pop() {
        let rr = Arc::clone(&rr);
        std::thread::spawn(move || loop {
            let (proxy_now, _i) = proxy_now.clone();
            let ret = run_checkers(cap_check_wait, proxy_now.to_owned());
            {
                let mut lock = match rr.lock() {
                    Ok(guard) => guard,
                    Err(poison) => poison.into_inner(),
                };
                match ret {
                    Ok(x) => {
                        debug!("CHK | OK cap {} checked {:?}", proxy_now, x);
                        let (tries, ms) = x;
                        RETRY_CNT.with_label_values(&[&proxy_now]).set(tries as i64);
                        ACCESS_TIME.with_label_values(&[&proxy_now]).set(ms as i64);
                        IS_ALIVE.with_label_values(&[&proxy_now]).set(1 as i64);
                        lock.change_state(&proxy_now, true);
                    }
                    Err(x) => {
                        debug!("CHK | ERR cap {} error {:?}", proxy_now, x);
                        IS_ALIVE.with_label_values(&[&proxy_now]).set(0 as i64);
                        lock.change_state(&proxy_now, false);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(cap_check_period));
        });
    }

    hyper::rt::run(hyper::rt::lazy(move || {
        let server = Server::bind(&in_addr).serve(proxy).map_err(|e| println!("Can not bind server: {}", e));
        hyper::rt::spawn(server);
        println!("Listening on http://{}", in_addr);
        Ok(())
    }));
}
