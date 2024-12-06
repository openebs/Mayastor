# Building and deploying Mayastor Helm Chart from scratch

This guide will walk you through the process of building all Mayastor images using Nix and Docker.
Once these are ready we can then install the helm chart using our freshly baked images.

Mayastor is a multi-component [Rust][rust-lang] project that makes heavy use of
[Nix][nix-explore] for our development and build process.

If you're coming from a non-Rust (or non-Nix) background, **building Mayastor may be a bit
different than you're used to.** There is no `Makefile`, you won't need a build toolchain,
you won't need to worry about cross compiler toolchains, and all builds are reproducible.

## Table of Contents

- [Prerequisites](#prerequisites)
    - [Build system](#build-system)
    - [Source Code](#source-code)
- [Building and Pushing](#building-and-pushing)
    - [Building](#building)
    - [Pushing](#pushing)
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

> **_NOTE_**:
> Some say we should have remained as a mono-repo :)

In order to build all images, you may check out all repos, example:

```bash
mkdir ~/mayastor && cd ~/mayastor
git clone --recurse-submodules https://github.com/openebs/mayastor.git -- io-engine
git clone --recurse-submodules https://github.com/openebs/mayastor-control-plane.git -- controller
git clone --recurse-submodules https://github.com/openebs/mayastor-extensions.git -- extensions
```

## Building and Pushing

Each of the repos contains a script for building and pushing all their respective container images.
Usually this is located at `./scripts/release.sh`
The api for this script is generally the same as it leverages a common [base script][deps-base-release.sh].

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

### Building

In order to build all images, we simply need to walk the repos.

If you want to see what happens under the hood, without building, you can use the `--dry-run`.

```bash
cd ~/mayastor
for repo in */; do
  $repo/scripts/release.sh --dry-run --alias-tag my-tag
done
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
> **_NOTE_**: This will also build the static kubectl-mayastor plugin. You can avoid that with `--skip-bins`.

```bash
cd ~/mayastor
for repo in */; do
  $repo/scripts/release.sh --skip-publish --alias-tag my-tag
done
```

After some time (14 minutes on my system), all images should be built.
The output is too long, but since the script loads the images locally, you can see them:

```bash
> docker image ls | grep my-tag
openebs/mayastor-fio-spdk                     my-tag    e577a4ae779b   49 minutes ago      142MB
openebs/mayastor-casperf                      my-tag    67b1997768fb   49 minutes ago      149MB
openebs/mayastor-io-engine                    my-tag    62c58727a721   54 minutes ago      315MB
openebs/mayastor-upgrade-job                  my-tag    4cb14b214fb4   54 minutes ago      154MB
openebs/mayastor-obs-callhome-stats           my-tag    d4f78e2c57fd   54 minutes ago      71.3MB
openebs/mayastor-obs-callhome                 my-tag    d44b0444883f   54 minutes ago      378MB
openebs/mayastor-metrics-exporter-io-engine   my-tag    416c1cfd3a64   56 minutes ago      60MB
openebs/mayastor-csi-node                     my-tag    50beac3984ee   58 minutes ago      408MB
openebs/mayastor-csi-controller               my-tag    4186d4ea8fbe   59 minutes ago      68.8MB
openebs/mayastor-api-rest                     my-tag    9bccb64557e0   59 minutes ago      70.4MB
openebs/mayastor-operator-diskpool            my-tag    d6f19ec945e0   59 minutes ago      67.5MB
openebs/mayastor-agent-ha-cluster             my-tag    146ed2c49a78   59 minutes ago      66.1MB
openebs/mayastor-agent-ha-node                my-tag    0a3f9b375ebd   59 minutes ago      103MB
openebs/mayastor-agent-core                   my-tag    938f7b481b2e   About an hour ago   87.6MB
```

### Pushing

You can push the images to your required registry/namespace using the argument `--registry`.\
For the purposes of this, we'll push my docker.io namespace: `docker.io/tiagolobocastro`.

```bash
cd ~/mayastor
for repo in */; do
  $repo/scripts/release.sh --registry docker.io/tiagolobocastro --alias-tag my-tag
done
```

> _**NOTE**_:
> If you don't specify the namespace, the default openebs namespace is kept.

### Installing

Installing the helm chart with the custom images is quite simple.

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
> helm install mayastor chart -n mayastor --create-namespace --set="image.repo=tiagolobocastro,image.tag=my-tag" --wait
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

[rust-lang]: https://www.rust-lang.org/

[nix-explore]: https://nixos.org/explore.html

[nix-install]: https://nixos.org/download.html

[github-openebs]: https://github.com/openebs

[deps-base-release.sh]: https://github.com/openebs/mayastor-dependencies/blob/HEAD/scripts/release.sh
