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

Install the Edgee CLI as follows:
​
```shell
$ curl https://install.edgee.cloud | sh
```

Or via homebrew:

```shell
$ brew tap edgee-cloud/edgee
$ brew install edgee
```

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
```

### `edgee help`

This command lets you get help about existing commands, sub-commands, and their respective options:

```bash 
$ edgee help
Usage: edgee <COMMAND>
Commands:
  login       Log in to the Edgee Console
  whoami      Print currently login informations
  components  Components management commands  [aliases: component]
  serve       Run the Edgee server [aliases: server]
```

### `edgee component[s]`

This command includes a few sub-commands that let you create, build, test, and push components.

#### `edgee components new`

This command lets you create a component in a new directory (including sample code)

```bash 
$ edgee components new
? Enter the name of the component: my-component
? Select the language of the component:
  C
  CSharp
  Go
  JavaScript
  Python
> Rust
  TypeScript
Downloading sample code for Rust...
Extracting code...
New project my-component setup, check README to install the correct dependencies.
```

#### `edgee components build`

This command lets you compile the component in the current folder into a WebAssembly binary file.

You can customize the behavior of the build command in the manifest file by changing the target file name
and the default build script. If you've created a new component with `edgee component new` the default build script
should be a great starting point. By default, the output of this command will be a new .wasm file in the current folder.


#### `edgee components check`

This command lets you validate the local .wasm file to make sure it's compliant with the WIT interface.

#### `edgee components test`

This command lets you run the local .wasm file with a sample event and provided settings.
This helps ensure your component behaves as expected from the proxy's perspective, in addition to your unit tests.

```bash
$ edgee components test \
    --event-type page \
    --settings "setting1=value1,setting2=value2"
```

#### `edgee components push`

This command lets you push the local .wasm file to the Edgee Component Registry.

```shell
$ edgee components push
? Component org/name does not exists, do you want to create it? Y/n
? Would you like to make this component public or private?
> private
  public
Component created successfully!
You can view and edit it at: https://edgee.cloud/~/registry/{organization}/{component}
```

### `edgee serve`

Learn more about [running the Edgee proxy locally](./README-proxy.md).


## Contributing
If you're interested in contributing to Edgee, read our [contribution guidelines](./CONTRIBUTING.md)

## Reporting Security Vulnerabilities
If you've found a vulnerability or potential vulnerability in our code, please let us know at
[edgee-security](mailto:security@edgee.cloud).

## Versioning
Edgee releases and their associated binaries are available on the project's [releases page](https://github.com/edgee-cloud/edgee/releases).

The binaries are versioned following [SemVer](https://semver.org/) conventions.
