#[allow(unused_imports)]
use log::{debug, info, warn};

use std::time::Instant;
use std::collections::HashMap;
use rand::Rng;

#[derive(Clone, Debug)]
pub struct Stat {
    pub url: String,
    pub dt_start: Instant,
}

#[derive(Debug)]
pub struct Prox {
    url: String,
    date: std::time::Instant,
}
#[derive(Debug)]
pub struct ProxUrl {
    url: String,
    weight: usize,
    enabled: bool,
}

#[derive(Debug)]
pub struct Proxies {
    pub urls: Vec<ProxUrl>,
    pub list: HashMap<usize, Prox>,
}

impl Proxies {
    pub fn get_proxy(&self) -> String {
        let mut rng = rand::thread_rng();
        let sum_weight = self.urls.iter().filter(|f| f.enabled).fold(0, |acc, x| acc + x.weight);
        let random = rng.gen_range(0, sum_weight - 1);

        let mut now: usize = 0;
        let mut final_result = self.urls[rng.gen_range(0, self.urls.len())].url.to_owned();
        for i in self.urls.iter() {
            now += i.weight;
            if now >= random && i.enabled {
                final_result = i.url.to_owned();
                break;
            }
        }
        final_result.to_owned()
    }
    pub fn change_state(&mut self, disable_url: &str, b: bool) {
        for url in self.urls.iter_mut() {
            if *url.url == *disable_url {
                url.enabled = b;
            }
        }
    }
    pub fn new(vec: Vec<(String, isize)>) -> Proxies {
        let urls = vec
            .iter()
            .map(|(u, w)| ProxUrl {
                url: u.to_string(),
                weight: w.abs() as usize,
                enabled: w >= &0,
            })
            .collect();
        Proxies {
            urls: urls,
            list: HashMap::new(),
        }
    }
    pub fn get(&mut self, id: usize) -> Option<String> {
        match self.list.get(&id) {
            Some(o) => {
                warn!( "OBJ | GET_TRUE item, id:{},  {} msec, {:?}", id, Instant::now().duration_since(o.date).as_millis(), o);
                Some(o.url.clone())
            }
            None => {
                warn!("OBJ | GET_FALSE item, id:{}", id);
                None
            }
        }
    }
    pub fn set(&mut self, id: usize, url: String) {
        let prox = Prox {
            url: url,
            date: Instant::now(),
        };
        self.list.insert(id, prox);

        // cleanup
        let now = Instant::now();
        let r2del: Vec<usize> = self.list.iter()
            .filter(|&(_, v)| now.duration_since(v.date).as_secs() > 60)
            .map(|(k, _)| k.to_owned())
            .collect();
        let _consumed: Vec<_> = r2del.iter().map(|i| self.list.remove(i)).collect();
        debug!("OBJ | R2D | {:?}", r2del);
    }
}
