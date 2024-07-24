<div align="center">

<p align="center">
  <a href="https://www.edgee.cloud">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://cdn.edgee.cloud/img/favicon-dark.svg">
      <img src="https://cdn.edgee.cloud/img/favicon.svg" height="100" alt="Edgee">
    </picture>
    <h1 align="center">Edgee</h1>
  </a>
</p>


**The full-stack edge platform for your edge oriented applications.**

[![Edgee](https://img.shields.io/badge/edgee-open%20source-blueviolet.svg)](https://www.edgee.cloud)
[![Crates.io](https://img.shields.io/crates/v/edgee.svg?logo=rust)](https://crates.io/crates/edgee)
[![FlakeHub](https://img.shields.io/endpoint?url=https://flakehub.com/f/edgee-cloud/edgee/badge)](https://flakehub.com/flake/edgee-cloud/edgee)
[![Docker](https://img.shields.io/docker/v/edgeecloud/edgee.svg?logo=docker&label=docker&color=0db7ed)](https://hub.docker.com/r/edgeecloud/edgee)
[![Edgee](https://img.shields.io/badge/slack-edgee-blueviolet.svg?logo=slack)](https://www.edgee.cloud/slack)
[![Docs](https://img.shields.io/badge/docs-published-blue)](https://docs.edgee.cloud)

</div>

<!-- TODO: Add FAQ -->
<!-- TODO: Add Video introduction -->
## Documentation
- [Official Website](https://www.edgee.cloud)
- [Official Documentation](https://docs.edgee.cloud)

## Contact
- [Twitter](https://x.com/edgee_cloud)
- [Slack](https://www.edgee.cloud/slack)

Make sure you've read [the official docs](https://docs.edgee.cloud) if you want to understand the main concepts and the architecture of Edgee.

# Running Edgee

The next section will explain how to configure Edgee proxy, for now let's assume you already have a valid configuration
file. For all examples we're gonna assume that TLS certificates can be found in `/var/edgee/cert` and WebAssembly
components in `/var/edgee/wasm`.

## Using docker

You can run it using the CLI
```console
docker run \
  -v $PWD/edgee.toml:/app/edgee.toml \
  -v $PWD/cert:/var/edgee/cert \
  -v $PWD/wasm:/var/edgee/wasm \
  -p80:80 \
  -p443:443 \
  edgeeecloud/edgee
```

or as part of a `docker-compose.yml`
```yaml
service:
  edgee:
    image: edgeecloud/edgee
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - "./edgee.toml:/app/edgee.toml"
      - "./cert:/var/edgee/cert"
      - "./wasm:/var/edgee/wasm"
```

## Using Nix Flakes

You can add Edgee to your nix by importing it from github
```nix
{
  inputs.edgee.url = "github:edgee-cloud/edgee";
}
```

We're also published on Flakehub
```nix
{
  inputs.edgee.url = "https://flakehub.com/f/edgee-cloud/edgee/*.tar.gz";
}
```

When running with Nix, make sure to run the server from the same directory as the configuration file.

## Running as a crate

Edgee is built in Rust and can be installed as a crate.

```console
cargo install edgee
```

When installing from Crates.io, make sure to run the server from the same directory as the configuration file.

# Configuration


Edgee proxy is customized through the `edgee.toml` file, which is expected to be present in the same directory where edgee is running from.

Here's a minimal configuration sample. The configuration sets Edgee to work as a regular reverse proxy. After
understanding this simple configuration we'll see how to enable edge components.

```toml
# edgee.toml
[log]
level = "warn"

[monitor]
address = "0.0.0.0:8222"

[http]
address = "0.0.0.0:80"
force_https = true

[https]
address = "0.0.0.0:80"
cert = "/var/edgee/cert/server.pem"
key = "/var/edgee/cert/edgee.key"

[[routing]]
domain = "demo.edgee.dev"

[[routing.backends]]
name = "demo"
default = true
address = "192.168.0.10"
enable_ssl = true
```

## Log levels
Edgee allows you to control the granularity of logs you want to be displayed. The possible values are:
`trace`, `debug`, `info`, `warn`, `error`, and `fatal`. This setting defines the minimal level to
be displayed, so setting it to `warn` will show `warn`, `error`, and `fatal` messages while hidding the others.

## Monitoring
The **monitor** entry point exposes the monitoring and observability features. We plan to implement support for
the popular observability frameworks in the future. For now it only exposts the `/healthz` HTTP endpoint to 
be used for health checking.

## Routing
Our example sets up one backend for this project, called "demo". Since it's the default backend, all traffic 
directed to `demo.edgee.dev` will be redirected there. Every project can have a number of backends and use 
routing rules to distribute traffic among them.

We can add a second backend called "api" and redirect there all requests to `demo.edgee.dev/api`.
```toml
# edgee.toml
[[routing.rules]]
path_prefix = "/api/"
backend = "api"

[[routing.backends]]
name = "api"
enable_ssl = true
address = "192.168.0.30"
```

The supported matchers are:
- *path*: the path matches exactly the provided value
- *path_prefix*: the path starts with the provided value
- *path_regexp*: the path matches the provided regexp

In addition to proxying the request it's also possible to rewrite the path:

```toml
# edgee.toml

[[routing.rules]]
path_prefix = "/api/"
rewrite = "/v1/"
backend = "api"
```

## Integrating with edgee components

Make sure you check the [official docs](https://docs.edgee.cloud/components/overview) to understand better what
components are.

In this example we're gonna implement data collection using the [amplitude component](https://github.com/edgee-cloud/amplitude-component).

The only thing you need to do to enable a data collection is to add a new session to your configuration pointing
to the WebAssembly component that implenebts the data collection protocol.

```toml
# edgee.toml
[[destinations.data_collection]]
name = "amplitude"
component = "/var/edgee/wasm/amplitude.wasm"
credentials.amplitude_api_key = "YOUR-API-KEY"
```

Edgee proxy doesn't ship with any builtin components. Here's a list of the current open source components we've built:
- [Amplitude](https://github.com/edgee-cloud/amplitude-component)
- [Google Analytics](https://github.com/edgee-cloud/ga-component)
- [Segment](https://github.com/edgee-cloud/segment-component)

You just need to place the WebAssembly in a know place and point to it in the configuration. You may also build your
own components for integrations we don't provide yet.

## Contributing
If you're interested in contributing to Edgee, read our [contribution guidelines](./CONTRIBUTING.md)

## Reporting Security Vulnerabilities
If you've found a vulnerability or potential vulnerability in our code, please let us know at
[edgee-security](mailto:security@edgee.cloud).
