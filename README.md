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


**The full-stack edge platform for your edge-oriented applications.**

[![Edgee](https://img.shields.io/badge/edgee-open%20source-blueviolet.svg)](https://www.edgee.cloud)
[![Docker](https://img.shields.io/docker/v/edgeecloud/edgee.svg?logo=docker&label=docker&color=0db7ed)](https://hub.docker.com/r/edgeecloud/edgee)
[![Edgee](https://img.shields.io/badge/slack-edgee-blueviolet.svg?logo=slack)](https://www.edgee.cloud/slack)
[![Docs](https://img.shields.io/badge/docs-published-blue)](https://docs.edgee.cloud)

</div>

⚠️ Edgee OSS Edition (v0.3.0) is in Development

Edgee OSS is currently in version 0.3.X and is considered unstable as we continue to enhance and refine the platform. 
We're actively working towards a stable v1.0.0 release, which will be available in the coming months. 
We welcome feedback and contributions during this development phase, and appreciate your patience as we work hard to bring you a robust edge computing solution.

<!-- TODO: Add FAQ -->
<!-- TODO: Add Video introduction -->
## Documentation
- [Official Website](https://www.edgee.cloud)
- [Official Documentation](https://docs.edgee.cloud)

## Contact
- [Twitter](https://x.com/edgee_cloud)
- [Slack](https://www.edgee.cloud/slack)

Check out [the official docs](https://docs.edgee.cloud) to dive into Edgee's main concepts and architecture.

# Running Edgee

Once you have a valid configuration file (see next section), you can run Edgee in different ways, using Docker, or running as a Rust crate.

⚠️ Note: all the examples below assume that TLS certificates and WebAssembly components can be found in `/var/edgee/cert` and  in `/var/edgee/wasm` respectively.

## Using docker

You can run it using the CLI:

```console
docker run \
  -v $PWD/edgee.toml:/app/edgee.toml \
  -v $PWD/cert:/var/edgee/cert \
  -v $PWD/wasm:/var/edgee/wasm \
  -p80:80 \
  -p443:443 \
  edgeeecloud/edgee
```

Or as part of a `docker-compose.yml`:

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

## Building from source

Edgee is built in Rust and can be installed using Cargo:

```console
cargo build --release
```

Then you can run it:

```console
cargo run --release
```

# Configuration

Edgee proxy is customized through the `edgee.toml` file (or `edgee.yaml`), which is expected to be present in the same directory where edgee is running from.

Here's a minimal configuration sample that sets Edgee to work as a regular reverse proxy. Later we'll see how to enable edge components.

```toml
# edgee.toml
[log]
level = "info"

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

## Routing
The example above sets up one backend called "demo". As the default backend, it will receive all traffic directed to `demo.edgee.dev`. Additionaly, projects can have a number of backends and use routing rules to distribute traffic among them.

For example, we could add a second backend called "api" to handle all requests to `demo.edgee.dev/api`:

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

In addition to proxying the request, you could also rewrite the path:

```toml
# edgee.toml

[[routing.rules]]
path_prefix = "/api/"
rewrite = "/v1/"
backend = "api"
```

## Integrating with edgee components

Check out the [official components docs](https://docs.edgee.cloud/components/overview) to dive into the
components architecture.

The Edgee proxy is designed for performance and extensibility, so you can easily integrate open source components based on the platforms you need. Here's a list of the components we've built so far:
- [Amplitude](https://github.com/edgee-cloud/amplitude-component)
- [Google Analytics](https://github.com/edgee-cloud/ga-component)
- [Segment](https://github.com/edgee-cloud/segment-component)

You just need point to the WebAssembly implementation in your proxy configuration. You may also build your
own components for integrations we don't provide yet.

### Example

Let's see how to implement data collection using the [amplitude component](https://github.com/edgee-cloud/amplitude-component).

You simply need to add a new session to your configuration pointing to the WebAssembly component that implements the data collection protocol:

```toml
# edgee.toml
[[components.data_collection]]
name = "amplitude"
component = "/var/edgee/wasm/amplitude.wasm"
credentials.amplitude_api_key = "YOUR-API-KEY"
```


## Contributing
If you're interested in contributing to Edgee, read our [contribution guidelines](./CONTRIBUTING.md)

## Reporting Security Vulnerabilities
If you've found a vulnerability or potential vulnerability in our code, please let us know at
[edgee-security](mailto:security@edgee.cloud).

## Versioning
Edgee releases and their associated binaries are available on the project's [releases page](https://github.com/edgee-cloud/edgee/releases).

The binaries are versioned following [SemVer](https://semver.org/) conventions.
