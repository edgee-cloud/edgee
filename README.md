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
[![Docs](https://img.shields.io/badge/docs-published-blue)](https://www.edgee.cloud/docs/introduction)
[![Coverage Status](https://coveralls.io/repos/github/edgee-cloud/edgee/badge.svg)](https://coveralls.io/github/edgee-cloud/edgee)

</div>

⚠️ Edgee Open Source Edition (v0.8.X) is in Development

Please consider this repository unstable as we continue to enhance and refine the platform.
We're actively working towards a stable v1.0.0 release, which will be available in the coming weeks.

Feedback and contributions are welcome during this development phase: we appreciate your patience as we work hard to bring you robust tooling for a great development experience.

### Useful resources

- Edgee's [Website](https://www.edgee.cloud), [Roadmap](https://www.edgee.cloud/roadmap), and [Documentation](https://www.edgee.cloud/docs/introduction)
- Edgee's [Community Slack](https://www.edgee.cloud/slack)
- Edgee on [X](https://x.com/edgee_cloud) and [LinkedIn](https://www.linkedin.com/company/edgee-cloud/)


## Getting started with the Edgee CLI

The Edgee CLI lets you create and build Wasm components locally with commands such as `edgee components new` and `edgee components build`.
When your component is ready, the Edgee CLI lets you push it to the Edgee Component Registry as a public or private component under your organization’s account, with `edgee components push`. Under the hood, the CLI interacts with the Edgee API and its goal is to simplify the local development experience across all supported languages.

Install the Edgee CLI with your preferred method:

<details open>
  <summary>Install script</summary>

  ```shell
  $ curl https://install.edgee.cloud | sh
  ```

</details>

<details>
  <summary>Homebrew</summary>

  ```shell
  $ brew tap edgee-cloud/edgee
  $ brew install edgee
  ```

</details>

<details>
  <summary>Cargo binstall</summary>

  ```shell
  $ cargo binstall edgee
  ```

</details>

<details>
  <summary>From source</summary>

  ```shell
  $ git clone https://github.com/edgee-cloud/edgee.git
  $ cd edgee
  $ cargo build --release
  $ ./target/release/edgee --version
  ```

</details>


## Edgee CLI commands

### `edgee login`

This command lets you log in using your Edgee account's API token (you can [create one here](https://www.edgee.cloud/~/me/settings/tokens)):


```shell
$ edgee login
Enter Edgee API token (press Ctrl+R to toggle input display): ****
```

### `edgee whoami`

This command lets you verify that the API is working correctly:

```shell
$ edgee whoami
Logged in as:
  ID:    XYZ-XYZ-DYZ
  Name:  Your name
  Email: your@email.com
  Url:   https://api.edgee.app
```

### `edgee help`

This command lets you get help about existing commands, sub-commands, and their respective options:

```bash
$ edgee help
Usage: edgee <COMMAND>
Commands:
  login                      Log in to the Edgee Console
  whoami                     Print currently login informations
  components                 Components management commands [aliases: component]
  serve                      Run the Edgee server [aliases: server]
  self-update                Update the Edgee executable
  generate-shell-completion  Print auto-completion script for your shell init file
  help                       Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### `edgee self-update`

This command lets you update the CLI to latest, if a new version is available.

Note: this only works if you've installed the CLI via the installation script above.

### `edgee component[s]`

This command includes a few sub-commands that let you create, build, test, and push components.

#### `edgee components new`

This command lets you create a component in a new directory (including sample code)

```bash
$ edgee components new
? Enter the component name: my-component
? Select a programming language:
  C
  CSharp
  Go
  JavaScript
  Python
> Rust
  TypeScript
 INFO Downloading sample code for Rust...
 INFO Extracting code...
 INFO Downloading WIT files...
 INFO New project my-component is ready! Check README for dependencies.
```

You can also use command arguments to skip the prompts

```bash
$ edgee components new --name foo --language javascript
 INFO Downloading sample code for JavaScript...
 INFO Extracting code...
 INFO Downloading WIT files...
 INFO New project foo is ready! Check README for dependencies.
```

#### `edgee components build`

This command lets you compile the component in the current folder into a WebAssembly binary file.

You can customize the behavior of the build command in the
[manifest file](https://www.edgee.cloud/docs/services/registry/developer-guide#component-manifest-file)
by changing the target file name
and the default build script. If you've created a new component with `edgee component new` the default build script
should be a great starting point. By default, the output of this command will be a new .wasm file in the current folder.


#### `edgee components check`

This command lets you validate the local .wasm file to make sure it's compliant with the WIT interface.

Note: this command runs automatically on push.

#### `edgee components test`

This command lets you run the local .wasm file with a sample event and provided settings.
This helps ensure your component behaves as expected from the proxy's perspective, in addition to your unit tests.

```bash
$ edgee components test \
    --event-type page \
    --settings "setting1=value1,setting2=value2"
```

You can also run the actual HTTP request automatically:

```bash
$ edgee components test [options] --make-http-request

```

Or generate the corresponding cURL command:

```bash
$ edgee components test [options] --curl
```

#### `edgee components push`

This command lets you push the local .wasm file to the Edgee Component Registry.

```shell
$ edgee components push
 INFO Component org/name does not exists yet!
> Confirm new component creation? Y/n
? Would you like to make this component public or private?
> private
  public
> Describe the new version changelog (optional) [(e) to open nano, (enter) to submit]
> Ready to push org/name@version. Confirm? Y/n
 INFO Uploading Wasm file...
 INFO Creating new version...
 INFO org/name@version pushed successfully!
 INFO URL: https://www.edgee.cloud/~/registry/{organization}/{component}
```

The push command also lets you publish or unpublish an existing component via `--public` or `--private`.

### `edgee serve`

Learn more about [running the Edgee proxy locally](./README-proxy.md).

### `edgee generate-shell-completion`

This command allows you to generate a script for your shell adding auto-completion for the `edgee` command.

```shell
$ edgee generate-shell-completion [SHELL]
# supported value: bash, elvish, fish, powershell, zsh
```

If no argument it passed, the CLI will try to guess it based on the environment.

To install the completions, source them in your shell init file.

<details>
  <summary>bash</summary>

  ```shell
  # ~/.bashrc
  $ eval $(edgee generate-shell-completion bash)
  ```

</details>

<details>
  <summary>zsh</summary>

  ```shell
  # store the auto-completion in ~/.zsh/_edgee
  $ edgee generate-shell-completion zsh > ~/.zsh/_edgee

  # ~/.zshrc
  fpath=(~/.zsh $fpath)
  autoload -Uz compinit
  compinit -u
  # note: you might need to delete ~/.zcompdump/ first
  ```

</details>

<details>
  <summary>fish</summary>

  ```shell
  # ~/.config/fish/completions/edgee.fish
  $ edgee generate-shell-completion fish | source
  ```

</details>

<details>
  <summary>elvish</summary>

  ```shell
  $ edgee generate-shell-completion elvish >> ~/.config/elvish/rc.elv
  ```

</details>

<details>
  <summary>powershell</summary>

  ```shell
  > edgee generate-shell-completion powershell >> $profile
  > .$profile
  ```

</details>

## Contributing
If you're interested in contributing to Edgee, read our [contribution guidelines](./CONTRIBUTING.md)

## Reporting Security Vulnerabilities
If you've found a vulnerability or potential vulnerability in our code, please let us know at
[edgee-security](mailto:security@edgee.cloud).

## Versioning
Edgee releases and their associated binaries are available on the project's [releases page](https://github.com/edgee-cloud/edgee/releases).

The binaries are versioned following [SemVer](https://semver.org/) conventions.
