# Testing Mayastor Control Plane

In order to test Mayastor, you'll need to be able to [**run Mayastor**][doc-run],
follow that guide for persistent hugepages & kernel module setup.

Or, for ad-hoc:

- Ensure at least 3072 2 MiB hugepages.

  ```bash
  echo 3072 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
  ```

- Ensure several kernel modules are installed:

  ```bash
  modprobe xfs nvme_fabrics nvme_tcp nvme_rdma
  ```

- Ensure docker is installed and the service is running (OS specific)

## Table of Contents

- [Table of Contents](#table-of-contents)
- [Local Docker Playground](#local-docker-playground)
  - [Deploying](#deploying)
- [Running the test suites](#running-the-test-suites)
  - [Unit/Integration/Docs](#unitintegrationdocs)
  - [BDD](#bdd)
  - [Testing with a custom io-engine](#testing-with-a-custom-io-engine)
- [Local K8s Playground](#local-k8s-playground)
  - [Example](#example)

## Local Docker Playground

The Mayastor integration tests leverage docker in order to create a "cluster" with multiple components running as their own docker container within the same network.
Specifically, the control-plane integration tests make use of the [deployer](https://github.com/openebs/mayastor-control-plane/blob/HEAD/deployer/README.md) which can setup these "clusters" for you, along with a very extensive range of options.

### Deploying

Starting a deployer "cluster", is then very simple:

```console
deployer start -s -i 2 -w 5s
[/core] [10.1.0.3] /home/tiago/git/mayastor/controller/target/debug/core --store etcd.cluster:2379
[/etcd] [10.1.0.2] /nix/store/7fvflmxl9a8hfznsc1sddp5az1gjlavf-etcd-3.5.13/bin/etcd --data-dir /tmp/etcd-data --advertise-client-urls http://[::]:2379 --listen-client-urls http://[::]:2379 --heartbeat-interval=1 --election-timeout=5
[/io-engine-1] [10.1.0.5] /bin/io-engine -N io-engine-1 -g 10.1.0.5:10124 -R https://core:50051 --api-versions V1 -r /host/tmp/io-engine-1.sock --ptpl-dir /host/tmp/ptpl/io-engine-1 -p etcd.cluster:2379
[/io-engine-2] [10.1.0.6] /bin/io-engine -N io-engine-2 -g 10.1.0.6:10124 -R https://core:50051 --api-versions V1 -r /host/tmp/io-engine-2.sock --ptpl-dir /host/tmp/ptpl/io-engine-2 -p etcd.cluster:2379
[/rest] [10.1.0.4] /home/tiago/git/mayastor/controller/target/debug/rest --dummy-certificates --https rest:8080 --http rest:8081 --workers=1 --no-auth
```

> **NOTE**: Use `--io-engine-isolate` to given each engine a different cpu core
> **NOTE**: Use `--developer-delayed` for sleep delay on each engine, reducing cpu usage
> **NOTE**: For all options, check `deployer start --help`

And with this we have a dual io-engine cluster which we can interact with.

```console
rest-plugin get nodes
 ID           GRPC ENDPOINT   STATUS  VERSION
 io-engine-2  10.1.0.6:10124  Online  v1.0.0-997-g17488f4a7da3
 io-engine-1  10.1.0.5:10124  Online  v1.0.0-997-g17488f4a7da3
```

You can also use the swagger-ui available on the [localhost:8081](http://localhost:8081/v0/swagger-ui#).

At the end of your experiment, remember to bring down the cluster:

```bash
deployer stop
```

## Running the test suites

> **TODO:** We're still writing this! Sorry! Let us know if you want us to prioritize this!

### Unit/Integration/Docs

Mayastor's unit tests, integration tests, and documentation tests via the conventional `cargo test`.

> **An important note**: Some tests need to run as root, and so invoke sudo.

> **Remember to enter the nix-shell before running any of the commands herein**

All tests share a deployer "cluster" and network and therefore this means they need to run one at a time.
Example, testing the `deployer-cluster` crate:

```bash
cargo test -p deployer-cluster -- --test-threads 1 --nocapture
```

To test all crates, simply use the provided script:

```bash
./scripts/rust/test.sh
```

### BDD

There is a bit of extra setup for the python virtual environment.

To prepare:

```bash
tests/bdd/setup.sh
```

Then, to run the tests:

```bash
./scripts/python/test.sh
```

If you want to run the tests manually, you can also do the following:

```bash
. tests/bdd/setup.sh # source the virtual environment
pytest tests/bdd/features/csi/node/test_parameters.py -x
```

### Testing with a custom io-engine

You can test with a custom io-engine by specifying environment variables:

- binary

    ```bash
    unset IO_ENGINE_BIN
    export IO_ENGINE_IMAGE=docker.io/tiagolobocastro/mayastor-io-engine:my-tag
    ```

- image

    ```bash
    unset IO_ENGINE_IMAGE
    export IO_ENGINE_BIN=~/mayastor/io-engine/target/debug/io-engine
    ```

## Local K8s Playground

If you need a K8s cluster, we have a [terraform] deployment available [here](https://github.com/openebs/mayastor-control-plane/tree/HEAD/terraform/cluster).
It can be used to deploy K8s on [libvirt] and [lxd].
> [!Warning]
> Please note that deployment on [lxd] is very experimental at the moment.\
> See for example: <https://github.com/openebs/mayastor/issues/1541>
>

> **TODO:** We're still writing this! Sorry! Let us know if you want us to prioritize this!\
> In the meantime, refer to the [README](https://github.com/openebs/mayastor-control-plane/tree/HEAD/terraform/cluster/README.adoc) for more help

### Example

```console
❯ terraform apply --var="worker_vcpu=4" --var="worker_memory=8192" --var="worker_nodes=3" --auto-approve
...
Apply complete! Resources: 25 added, 0 changed, 0 destroyed.

Outputs:

kluster = <<EOT
[master]
ksmaster-1 ansible_host=10.0.0.223 ansible_user=tiago ansible_ssh_private_key_file=/home/tiago/.ssh/id_rsa ansible_ssh_common_args='-o StrictHostKeyChecking=no'

[nodes]
ksworker-1 ansible_host=10.0.0.89 ansible_user=tiago ansible_ssh_private_key_file=/home/tiago/.ssh/id_rsa ansible_ssh_common_args='-o StrictHostKeyChecking=no'
ksworker-2 ansible_host=10.0.0.157 ansible_user=tiago ansible_ssh_private_key_file=/home/tiago/.ssh/id_rsa ansible_ssh_common_args='-o StrictHostKeyChecking=no'
ksworker-3 ansible_host=10.0.0.57 ansible_user=tiago ansible_ssh_private_key_file=/home/tiago/.ssh/id_rsa ansible_ssh_common_args='-o StrictHostKeyChecking=no'

EOT
```

At the end of your experiment, remember to bring down the cluster:

```console
❯ terraform destroy --auto-approve
```

[doc-run]: ./run.md

[terraform]: https://www.terraform.io/

[libvirt]: https://libvirt.org/

[lxd]: https://canonical.com/lxd
