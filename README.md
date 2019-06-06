[![Travis Build Status][travis-badge]][travis-url]
[![Docker][docker-badge]][docker-url]

[travis-badge]: https://api.travis-ci.org/arrrght/caproxy.svg?branch=master
[travis-url]: https://travis-ci.org/arrrght/caproxy

[docker-badge]: https://img.shields.io/docker/pulls/a3rght/caproxy.png
[docker-url]: https://cloud.docker.com/repository/docker/a3rght/caproxy

[CapMonster](https://zennolab.com/ru/products/capmonster/) balanced proxy

```
Metrics for prometheus:
http://127.0.0.1:8080/metrics

Listen at:
CAP_LISTEN(addr:port, default 0.0.0.0:8080)
CAP_LISTEN=1.2.3.4:9090

Log verbosity:
RUST_LOG=caproxy={trace|debug|log}
RUST_LOG=caproxy=debug

Cap hosts, balanced with score. If score is negative, it will be disabled on start.
CAPS={score1=url1}[,{score2}={url2}]
CAPS=20=http://cap1.org,80=http://cap2.net
CAPS=20=http://cap-one.org,-80=http://cap-second.net

CapMonster host check:
CAPS_CHECK_PERIOD(msec, default 5000): period between checks
CAPS_CHECK_WAIT(msec, default 200): wait for answer between NOT_READY
```
