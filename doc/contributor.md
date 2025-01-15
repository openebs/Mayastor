# Contributing to Mayastor

This guide will walk you through the process of building and testing all Mayastor components using Nix and Docker.

Mayastor is a multi-component [Rust][rust-lang] project that makes heavy use of
[Nix][nix-explore] for our development and build process.

If you're coming from a non-Rust (or non-Nix) background, **building Mayastor may be a bit
different than you're used to.** There is no `Makefile`, you won't need a build toolchain,
you won't need to worry about cross compiler toolchains, and all builds are reproducible.

Mayastor is a sub-project of [OpenEBS][github-openebs], so don't forget to checkout the [umbrella contributor guide](https://github.com/openebs/community/blob/HEAD/CONTRIBUTING.md).

## Table of Contents

- [Prerequisites](#prerequisites)
  - [Build system](#build-system)
  - [Test system](#test-system)
  - [Source Code](#source-code)
- [Building binaries](#building-binaries)
  - [Building local binaries](#building-local-binaries)
- [Testing](#testing)
  - [Mayastor I/O Engine (data-plane)](#mayastor-io-engine-data-plane)
  - [Mayastor Control Plane](#mayastor-control-plane)
  - [Mayastor Extensions](#mayastor-extensions)
  - [CI](#ci)
    - [Jenkins](#jenkins)
    - [GitHub Actions](#github-actions)
- [Deploying to K8s](#deploying-to-k8s)
  - [Building the images](#building-the-images)
  - [Pushing the images](#pushing-the-images)
  - [Iterative Builds](#iterative-builds)
  - [Installing](#installing)

## Prerequisites

Mayastor **only** builds on modern Linuxes. We'd adore contributions to add support for
Windows, FreeBSD, OpenWRT, or other server platforms.

If you do not have a Linux system:

- **Windows:** We recommend using [WSL2][windows-wsl2] if you only need to
  build Mayastor. You'll need a [Hyper-V VM][windows-hyperv] if you want to use it.
- **Mac:** We recommend you use [Docker for Mac][docker-install]
  and follow the Docker process described. Please let us know if you find a way to
  run it!
- **FreeBSD:** We _think_ this might actually work, SPDK is compatible! But, we haven't
  tried it yet.
- **Others:** This is kind of a "Do-it-yourself" situation. Sorry, we can't be more help!

### Build system

The only thing your system needs to build Mayastor is [**Nix**][nix-install].

Usually [Nix][nix-install] can be installed via (Do **not** use `sudo`!):

```bash
curl -L https://nixos.org/nix/install | sh
```

### Test system

### Source Code

Mayastor is split across different GitHub repositories under the [OpenEBS][github-openebs] organization.

Here's a breakdown of the required repos for the task at hand:

- **_data-plane_**: <https://github.com/openebs/mayastor>
  - The data-plane components:
    - io-engine (the only one which we need for this)
    - io-engine-client
    - casperf
- **_control-plane_**: <https://github.com/openebs/mayastor-control-plane>
  - Various control-plane components:
    - agent-core
    - agent-ha-cluster
    - agent-ha-node
    - operator-diskpool
    - csi-controller
    - csi-node
    - api-rest
- **_extensions_**: <https://github.com/openebs/mayastor-extensions>
  - Mostly K8s specific components:
    - kubectl-mayastor
    - metrics-exporter-io-engine
    - call-home
    - stats-aggregator
    - upgrade-job
  - Also contains the helm-chart

> **_NOTE_**:
> There are also a few other repositories which are pulled or submoduled by the repositories above

If you want to tinker with all repos, here's how you can check them all out:

```bash
mkdir ~/mayastor && cd ~/mayastor
git clone --recurse-submodules https://github.com/openebs/mayastor.git -- io-engine
git clone --recurse-submodules https://github.com/openebs/mayastor-control-plane.git -- controller
git clone --recurse-submodules https://github.com/openebs/mayastor-extensions.git -- extensions
```

## Building binaries

### Building local binaries

Each code repository contains it's own [`nix-shell`][nix-shell] environment and with it all pre-requisite build dependencies.

> **NOTE**
> To run the tests, you might need additional OS configuration, example: a docker service.

```bash
cd ~/mayastor/controller
nix-shell
```

Once entered, you can start any tooling (eg `code .`) to ensure the correct resources are available.
The project can then be interacted with like any other Rust project.

Building:

```bash
cargo build --bins
```

## Testing

There are a few different types of tests used in Mayastor:

- Unit Tests
- Component Tests
- BDD Tests
- E2E Tests
- Load Tests
- Performance Tests

Each repo may have a subset of the types defined above.

### Mayastor I/O Engine (data-plane)

Find the guide [here](./test.md).

### Mayastor Control Plane

Find the guide [here](./test-controller.md).

### Mayastor Extensions

Find the guide [here](./test-extensions.md).

### CI

Each repo has its own CI system which is based on [bors] and [GitHub Actions][github-actions].
At its core, each pipeline runs the Unit/Integration tests, the BDD tests and image-build tests, ensuring that a set of images can be built once a PR is merged to the target branch.

#### Jenkins [deprecated]

For the [Jenkins][jenkins] pipeline you can refer to the `./Jenkinsfile` on each branch.
The Jenkins systems are currently setup on the DataCore sponsored hardware and need to be reinstalled to CNCF sponsored hardware or perhaps even completely moved to GitHub Actions.

> _**Deprecated**_\
> CI has now fully migrated to GithubActions and Jenkins CI is now deprecated, and only setup for older release branches (up to release/2.7)

#### GitHub Actions

For the GitHub Actions you can refer to the `./github/workflows` on each repo.

Some actions run when a PR is created/updated, whilst others run as part of [bors]. \
Here are some examples of how to interact with bors:

| Syntax | Description |
|--------|------------ |
| bors r+ | Run the test suite and push to master if it passes. Short for "reviewed: looks good" |
| bors merge | Equivalent to `bors r+` |
| bors r=[list] | Same as r+, but the "reviewer" in the commit log will be recorded as the user(s) given as the argument |
| bors merge=[list] | Equivalent to `bors r=[list]` |
| bors r- | Cancel an r+, r=, merge, or merge= |
| bors merge- | Equivalent to `bors r-` |
| bors try | Run the test suite without pushing to master |
| bors try- | Cancel a try |
| bors delegate+ <br> bors d+ | Allow the pull request author to r+ their changes |
| bors delegate=[list] <br> bors d=[list] | Allow the listed users to r+ this pull request's changes |
| bors ping | Check if bors is up. If it is, it will comment with _pong_ |
| bors retry | Run the previous command a second time |
| bors p=[priority] | Set the priority of the current pull request. Pull requests with different priority are never batched together. The pull request with the bigger priority number goes first |
| bors r+ p=[priority] | Set the priority, run the test suite, and push to master (shorthand for doing p= and r+ one after the other) |
| bors merge p=[priority] | Equivalent to `bors r+ p=[priority]` |

## Deploying to K8s

When you're mostly done with a set of changes, you'll want to test them in a K8s cluster, and for this you need to build docker images.
Each of the repos contains a script for building and pushing all their respective container images.
Usually this is located at `./scripts/release.sh`
The api for this script is generally the same as it leverages a common [base script][deps-base-release.sh].

### Building the images

```bash
> ./scripts/release.sh --help
Usage: release.sh [OPTIONS]

  -d, --dry-run              Output actions that would be taken, but don't run them.
  -h, --help                 Display this text.
  --registry <host[:port]>   Push the built images to the provided registry.
                             To also replace the image org provide the full repository path, example: docker.io/org
  --debug                    Build debug version of images where possible.
  --skip-build               Don't perform nix-build.
  --skip-publish             Don't publish built images.
  --image           <image>  Specify what image to build and/or upload.
  --tar                      Decompress and load images as tar rather than tar.gz.
  --skip-images              Don't build nor upload any images.
  --alias-tag       <tag>    Explicit alias for short commit hash tag.
  --tag             <tag>    Explicit tag (overrides the git tag).
  --incremental              Builds components in two stages allowing for faster rebuilds during development.
  --build-bins               Builds all the static binaries.
  --no-static-linking        Don't build the binaries with static linking.
  --build-bin                Specify which binary to build.
  --skip-bins                Don't build the static binaries.
  --build-binary-out <path>  Specify the outlink path for the binaries (otherwise it's the current directory).
  --skopeo-copy              Don't load containers into host, simply copy them to registry with skopeo.
  --skip-cargo-deps          Don't prefetch the cargo build dependencies.

Environment Variables:
  RUSTFLAGS                  Set Rust compiler options when building binaries.

Examples:
  release.sh --registry 127.0.0.1:5000
```

If you want to see what happens under the hood, without building, you can use the `--dry-run`.

```bash
cd ~/mayastor/controller
./scripts/release.sh --dry-run --alias-tag my-tag
```

Here's a snippet of what you'd actually see:

```text
~/mayastor/controller ~/mayastor
nix-build --argstr img_tag my-tag --no-out-link -A control-plane.project-builder.cargoDeps
Cargo vendored dependencies pre-fetched after 1 attempt(s)
Building openebs/mayastor-agent-core:my-tag ...
nix-build --argstr img_tag my-tag --out-link agents.core-image -A images.release.agents.core --arg allInOne true --arg incremental false --argstr product_prefix  --argstr rustFlags
docker load -i agents.core-image
rm agents.core-image
Building openebs/mayastor-agent-ha-node:my-tag ...
nix-build --argstr img_tag my-tag --out-link agents.ha.node-image -A images.release.agents.ha.node --arg allInOne true --arg incremental false --argstr product_prefix  --argstr rustFlags
docker load -i agents.ha.node-image
rm agents.ha.node-image
Building openebs/mayastor-agent-ha-cluster:my-tag ...
nix-build --argstr img_tag my-tag --out-link agents.ha.cluster-image -A images.release.agents.ha.cluster --arg allInOne true --arg incremental false --argstr product_prefix  --argstr rustFlags
docker load -i agents.ha.cluster-image
```

If you want to build, but not push it anywhere, you can skip the publishing with `--skip-publish`.

> **_NOTE_**: For repos with static binaries, you can avoid building them with `--skip-bins`.

```bash
cd ~/mayastor/controller
./scripts/release.sh --skip-publish --alias-tag my-tag
```

> _**NOTE**:
> Take a look [here](./build-all.md) for the guide building and pushing all images

### Pushing the images

You can push the images to your required registry/namespace using the argument `--registry`.\
For the purposes of this, we'll push my docker.io namespace: `docker.io/tiagolobocastro`.

```bash
cd ~/mayastor/controller
./scripts/release.sh --registry docker.io/tiagolobocastro --alias-tag my-tag
```

> _**NOTE**_:
> If you don't specify the namespace, the default openebs namespace is kept.

### Iterative Builds

The default image build process attempts to build all images part of a single repo in one shot, thus reducing the build time.
If you're iterating over code changes on a single image, you may wish to enable the iterative build flag which will not rebuild the dependencies over and over again.

```bash
cd ~/mayastor/controller
./scripts/release.sh --registry docker.io/tiagolobocastro --alias-tag my-tag --image csi.controller --incremental
```

### Installing

Installing the full helm chart with the custom images is quite simple.

> _**NOTE**_:
> One last step is required, mostly due to a bug or unexpected behaviour with the helm chart. \
> We'll need to manually push this container image:
>
>```bash
>docker pull docker.io/openebs/alpine-sh:4.1.0
>docker tag docker.io/openebs/alpine-sh:4.1.0 docker.io/tiagolobocastro/alpine-sh:4.1.0
>docker push docker.io/tiagolobocastro/alpine-sh:4.1.0
>```

```bash
> helm install mayastor mayastor/mayastor -n mayastor --create-namespace --set="image.repo=tiagolobocastro,image.tag=my-tag" --wait
NAME: mayastor
LAST DEPLOYED: Fri Dec  6 15:42:16 2024
NAMESPACE: mayastor
STATUS: deployed
REVISION: 1
NOTES:
OpenEBS Mayastor has been installed. Check its status by running:
$ kubectl get pods -n mayastor

For more information or to view the documentation, visit our website at https://openebs.io/docs/
```

If you're only building certain components, you may want to modify the images of an existing deployment, or configure per-repo tags, example:

```bash
helm install mayastor mayastor/mayastor -n mayastor --create-namespace --set="image.repo=tiagolobocastro,image.repoTags.control-plane=my-tag" --wait
```

> _**NOTE**_:
> We are currently missing overrides for registry/namespace/image:tag on specific Mayastor components

[rust-lang]: https://www.rust-lang.org/

[nix-explore]: https://nixos.org/explore.html

[nix-shell]: https://nixos.org/manual/nix/unstable/command-ref/new-cli/nix3-shell.html

[windows-wsl2]: https://wiki.ubuntu.com/WSL#Ubuntu_on_WSL

[windows-hyperv]: https://wiki.ubuntu.com/Hyper-V

[docker-install]: https://docs.docker.com/get-docker/

[nix-install]: https://nixos.org/download.html

[github-openebs]: https://github.com/openebs

[deps-base-release.sh]: https://github.com/openebs/mayastor-dependencies/blob/HEAD/scripts/release.sh

[jenkins]: https://www.jenkins.io/

[github-actions]: https://docs.github.com/en/actions

[bors]: https://github.com/bors-ng/bors-ng
