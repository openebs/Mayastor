# Testing Mayastor Extensions

In order to test Mayastor, you'll need to be able to [**run Mayastor**][doc-run],
follow that guide for persistent hugepages & kernel module setup.

Or, for ad-hoc:

- Ensure at least 1024 2 MiB hugepages.

  ```bash
  echo 1024 | sudo tee /sys/kernel/mm/hugepages/hugepages-2048kB/nr_hugepages
  ```

- Ensure several kernel modules are installed:

  ```bash
  modprobe xfs nvme_fabrics nvme_tcp nvme_rdma
  ```

- Ensure docker is installed and the service is running (OS specific)

## Table of Contents

- [Table of Contents](#table-of-contents)
- [Local K8s Playground](#local-k8s-playground)
  - [Deploying](#deploying)
- [Running the test suites](#running-the-test-suites)
  - [Unit/Integration/Docs](#unitintegrationdocs)

## Local K8s Playground

The Mayastor extensions repo uses [kind](https://kind.sigs.k8s.io/) to setup multi-node K8s environments for simple local testing.

> [!Warning] _**Limitation**_\
> Kind deploys K8s nodes as docker containers on the same host, and thus sharing the same host's kernel
> Currently this means the HA feature becomes a little confusing as multiple nodes may start reporting path failures

A [helper script](https://github.com/openebs/mayastor-extensions/blob/HEAD/scripts/k8s/deployer.sh) is given allowing to even more easily deploy these clusters and pre-configure them for Mayastor.

> [!Warning] Kernel Modules\
> This script will attempt to install kernel modules

### Deploying

Starting a kind cluster, is then very simple:

```console
❯ ./scripts/k8s/deployer.sh start --workers 2 --label --disk 1G
Current hugepages (4096) are sufficient
nvme-tcp kernel module already installed
NVMe multipath support IS enabled
Creating cluster "kind" ...
 ✓ Ensuring node image (kindest/node:v1.30.0) 🖼
 ✓ Preparing nodes 📦 📦 📦
 ✓ Writing configuration 📜
 ✓ Starting control-plane 🕹
 ✓ Installing CNI 🔌
 ✓ Installing StorageClass 💾
 ✓ Joining worker nodes 🚜
Set kubectl context to "kind-kind"
You can now use your cluster with:

kubectl cluster-info --context kind-kind

Thanks for using kind! 😊
Kubernetes control plane is running at https://127.0.0.1:45493
CoreDNS is running at https://127.0.0.1:45493/api/v1/namespaces/kube-system/services/kube-dns:dns/proxy

To further debug and diagnose cluster problems, use 'kubectl cluster-info dump'.
HostIP: "172.18.0.1"
```

> **NOTE**:\
> Use `--disk` to specify the fallocated file size which can be used to create pools on.\
> Each disk is mounted on `/var/local/mayastor/io-engine/disk.io` on each worker.

And with this we have a dual worker node cluster which we can interact with.

```console
❯ kubectl get nodes
NAME                 STATUS   ROLES           AGE   VERSION
kind-control-plane   Ready    control-plane   15m   v1.30.0
kind-worker          Ready    <none>          14m   v1.30.0
kind-worker2         Ready    <none>          14m   v1.30.0
```

We also provide a [simple script](https://github.com/openebs/mayastor-extensions/blob/HEAD/scripts/helm/install.sh) to deploy a non-production mayastor helm chart for testing:

```console
❯ ./scripts/helm/install.sh --wait
Installing Mayastor Chart
+ helm install mayastor ./scripts/helm/../../chart -n mayastor --create-namespace --set=etcd.livenessProbe.initialDelaySeconds=5,etcd.readinessProbe.initialDelaySeconds=5,etcd.replicaCount=1 --set=obs.callhome.enabled=true,obs.callhome.sendReport=false,localpv-provisioner.analytics.enabled=false --set=eventing.enabled=false --wait --timeout 5m
NAME: mayastor
LAST DEPLOYED: Tue Dec 17 10:18:27 2024
NAMESPACE: mayastor
STATUS: deployed
REVISION: 1
NOTES:
OpenEBS Mayastor has been installed. Check its status by running:
$ kubectl get pods -n mayastor

For more information or to view the documentation, visit our website at https://openebs.io/docs/.
+ set +x
NAME                                            READY   STATUS            RESTARTS   AGE     IP           NODE           NOMINATED NODE   READINESS GATES
mayastor-agent-core-6bf75fc6f8-pclc2            2/2     Running           0          3m10s   10.244.2.5   kind-worker    <none>           <none>
mayastor-agent-ha-node-46jkk                    1/1     Running           0          3m10s   172.18.0.2   kind-worker    <none>           <none>
mayastor-agent-ha-node-ljbfj                    1/1     Running           0          3m10s   172.18.0.3   kind-worker2   <none>           <none>
mayastor-api-rest-7b4b575765-2lvqv              1/1     Running           0          3m10s   10.244.2.2   kind-worker    <none>           <none>
mayastor-csi-controller-66b784d69f-zzl6z        6/6     Running           0          3m10s   172.18.0.3   kind-worker2   <none>           <none>
mayastor-csi-node-flbdg                         2/2     Running           0          3m10s   172.18.0.3   kind-worker2   <none>           <none>
mayastor-csi-node-tqqc9                         2/2     Running           0          3m10s   172.18.0.2   kind-worker    <none>           <none>
mayastor-etcd-0                                 1/1     Running           0          3m10s   10.244.1.5   kind-worker2   <none>           <none>
mayastor-io-engine-6jlzq                        0/2     PodInitializing   0          3m10s   172.18.0.2   kind-worker    <none>           <none>
mayastor-io-engine-9vmsd                        2/2     Running           0          3m10s   172.18.0.3   kind-worker2   <none>           <none>
mayastor-localpv-provisioner-56dbcc9fb8-w7csf   1/1     Running           0          3m10s   10.244.1.3   kind-worker2   <none>           <none>
mayastor-loki-0                                 1/1     Running           0          3m10s   10.244.2.8   kind-worker    <none>           <none>
mayastor-obs-callhome-69c9c454f7-d6wqr          1/1     Running           0          3m10s   10.244.2.3   kind-worker    <none>           <none>
mayastor-operator-diskpool-7458c66b8-7s4z2      1/1     Running           0          3m10s   10.244.2.4   kind-worker    <none>           <none>
mayastor-promtail-2jq85                         1/1     Running           0          3m10s   10.244.1.2   kind-worker2   <none>           <none>
mayastor-promtail-9hzqt                         1/1     Running           0          3m10s   10.244.2.6   kind-worker    <none>           <none>
```

Now, you can list the io-engine nodes for example:

```console
❯ kubectl-mayastor get nodes
 ID            GRPC ENDPOINT     STATUS  VERSION
 kind-worker2  172.18.0.3:10124  Online  v1.0.0-997-g17488f4a7da3
 kind-worker   172.18.0.2:10124  Online  v1.0.0-997-g17488f4a7da3
```

At the end of your experiment, remember to bring down the cluster:

```console
❯ ./scripts/k8s/deployer.sh stop
Deleting cluster "kind" ...
Deleted nodes: ["kind-control-plane" "kind-worker2" "kind-worker"]
```

## Running the test suites

> [!Warning] _**Tests**_\
> Sadly, this repo is lacking in tests, any help here would be greatly welcomed!

### Unit/Integration/Docs

Mayastor's unit tests, integration tests, and documentation tests via the conventional `cargo test`.

> **Remember to enter the nix-shell before running any of the commands herein**

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
pytest tests/bdd/features/test_upgrade.py -k test_upgrade_to_vnext
```

[doc-run]: ./run.md
