# Running the Edgee proxy locally

Edgee acts as a reverse proxy in front of a website, intercepting HTTP requests and running business logic on top of edge networks and CDNs such as Fastly and Cloudflare; the proxy interacts with WebAssembly components to implement features such as data collection for analytics, warehousing, and attribution data.

Once you have a valid configuration file (see next section), you can run the Edgee proxy in different ways, using the installer, Docker or running as a Rust crate.

⚠️ Note: all the examples below assume that TLS certificates and WebAssembly components can be found in `/var/edgee/cert` and  `/var/edgee/wasm` respectively. Feel free to use `/local/cert` and `/local/wasm` for local development.

# Install the Edgee CLI

You can install and run `edgee` locally using the installer script:

```shell
$ curl https://install.edgee.cloud | sh
```

Or via homebrew:

```shell
$ brew tap edgee-cloud/edgee
$ brew install edgee
```

Once installed, you can run the local proxy as follows:

```shell
$ edgee serve
```


### Alternative installation methods

#### Using Docker

You can run it using the Docker CLI:

```shell
docker run \
  -v $PWD/edgee.toml:/app/edgee.toml \
  -v $PWD/cert:/var/edgee/cert \
  -v $PWD/wasm:/var/edgee/wasm \
  -p80:80 \
  -p443:443 \
  edgeecloud/edgee \
  serve
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

##### Note for macOS ARM chips

In case you encounter the error "no match for platform in manifest: not found", simply pull the image as follows:

```shell

docker pull edgeecloud/edgee:latest --platform linux/amd64
```

And then use the `docker run` command as usual.

#### Building from source

Build the Rust package using Cargo:

```console
cargo build --release
```

Then you can run the local proxy:

```console
cargo run --release serve
```

# Configuration

Edgee proxy is customized through the `edgee.toml` file (or `edgee.yaml`), which is expected in the current directory.

You can get started by coping the existing `edgee.sample.toml` file:

```bash
cp edgee.sample.toml edgee.toml
```

Here's a minimal sample configuration that sets Edgee to work as a regular reverse proxy. Later we'll see how to enable WebAssembly components.

```toml
# edgee.toml
[log]
level = "info"

[http]
address = "0.0.0.0:80"
force_https = true

[https]
address = "0.0.0.0:443"
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

This way, calling `/api/test` on port `80` will result in calling `/v1/test` on the API backend.

### Redirections

Edgee supports HTTP redirections, allowing you to redirect traffic temporarily from one URL to another. 

#### Example

Here's how you can set up a redirection in your `edgee.toml` configuration file:

```toml
# edgee.toml
[[routing.redirections]]
source = "/old-path"
target = "https://example.com/new-path"


[[routing.redirections]]
source = "/foo"
target = "/bar"
```

In this example, requests to `https://demo.edgee.dev/old-path` will be temporarily (HTTP 302) redirected to `https://example.com/new-path` and requests to `https://demo.edgee.dev/foo` will be redirected to `https://demo.edgee.dev/bar`



## Integrating with edgee components

Check out the [official components docs](https://www.edgee.cloud/docs/components/overview) to dive into the
components architecture.

The Edgee proxy is designed for performance and extensibility, so you can easily integrate open source components based on the platforms you need. Here's a list of the components we've built so far:
- [Amplitude](https://github.com/edgee-cloud/amplitude-component)
- [Google Analytics](https://github.com/edgee-cloud/ga-component)
- [Segment](https://github.com/edgee-cloud/segment-component)
- [Piano Analytics](https://github.com/edgee-cloud/piano-analytics-component)

You just need point to the WebAssembly implementation in your proxy configuration. You may also build your
own components for integrations we don't provide yet.

### Example

Let's see how to implement data collection using the [amplitude component](https://github.com/edgee-cloud/amplitude-component).

You simply need to add a new session to your configuration pointing to the WebAssembly component that implements the data collection protocol:

```toml
# edgee.toml
[[components.data_collection]]
id = "amplitude"
file = "/var/edgee/wasm/amplitude.wasm"
settings.amplitude_api_key = "YOUR-API-KEY"
```

### Debugging a component

You can enable debug logs for a specific component by setting the `debug` flag to `true`:

```bash
./edgee --debug-component amplitude serve
```
