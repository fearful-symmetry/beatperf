## beatperf

A simple rust tool for reading and graphing metrics from beat's perf endpoint

### Requirements

The plotter library requires fontconfig and libfontconfig:

```
sudo apt install pkg-config libfreetype6-dev libfontconfig1-dev
```

### Usage

To enable metric reporting set `http.enabled: true` in the beat config.
`beatperf` is fairly simple:

```
Usage: beatperf [OPTIONS] <--metrics <METRICS>|--memory|--cpu|--processdb|--pipeline|--output|--ndjson <NDJSON>|--kernel-tracing> [ENDPOINT]

Arguments:
  [ENDPOINT]  the hostname:port combination of the beat stat endpoint [default: localhost:5066]

Options:
  -i, --interval <INTERVAL>  How often to fetch stats, in seconds [default: 5]
  -m, --metrics <METRICS>    A list of custom metrics to monitor, in dot-notation
      --memory               report memory metrics
      --cpu                  report CPU metrics
      --processdb            report add_session_metadata's processDB metrics
      --pipeline             report libbeat pipeline metrics
      --kernel-tracing       report add_sesson_metadata's kernel_tracing metrics
      --output               Report output event metrics
  -v, --verbose              Debug logging
      --ndjson <NDJSON>      dump all beat metrics to an ndjson file
      --read <READ>          Read metrics from an file, instead of from a a beat http endpoint
  -h, --help                 Print help
  -V, --version              Print version
```

For example, to monitor memory and cpu metrics:

```
beatperf -i 5 --cpu --memory
```

You can also read and write to an ndjson file:

```
beatperf --memory --ndjson output.ndjson
```

```
beatperf --memory --read output.ndjson
```

generate a graph from a pre-existing ndjson file:
```
beatperf  -i 3 -v --memory --read output.ndjson
```