# DiskPool Custom Resource for K8s

The DiskPool operator is a [K8s] specific component which manages pools in a K8s environment. \
Simplistically, it drives pools across the various states listed below.

In [K8s], mayastor pools are represented as [Custom Resources][k8s-cr], which is an extension on top of the existing [K8s API][k8s-api]. \
This allows users to declaratively create [diskpool], and mayastor will not only eventually create the corresponding mayastor pool but will
also ensure that it gets re-imported after pod restarts, node restarts, crashes, etc...

> **NOTE**: mayastor pool (msp) has been renamed to diskpool (dsp)

## DiskPool States

> *NOTE*
> Non-exhaustive enums could have additional variants added in the future. Therefore, when matching against variants of non-exhaustive enums, an extra
> wildcard arm must be added to account for future variants.

- Creating \
The pool is a new OR missing resource, and it has not been created or imported yet. The pool spec ***MAY*** be present but ***DOES NOT*** have a status field.

- Created \
The pool has been created in the designated i/o engine node by the control-plane.

- Terminating \
A deletion request has been issued by the user. The pool will eventually be deleted by the control-plane and eventually the DiskPool Custom Resource will also get removed from the K8s API.

- Error (*Deprecated*) \
The attempt to transition to the next state has exceeded the maximum number of retries. The retry counts are implemented using an exponential back-off, which by default is set to 10. Once the error state is entered, reconciliation stops. Only external events (a new resource version) will trigger a new attempt.
  > NOTE: this State has been deprecated since API version **v1beta1**

## Reconciler actions

The operator responds to two types of events:

- Scheduled \
When, for example, we try to submit a new PUT request for a pool. On failure (i.e., network) we will reschedule the operation after 5 seconds.

- CRD updates \
When the CRD is changed, the resource version is changed. This will trigger a new reconcile loop. This process is typically known as “watching.”

- Observability \
During the transition, the operator will emit events to K8s, which can be obtained by kubectl. This gives visibility into the state and its transitions.

[K8s]: https://kubernetes.io/
[k8s-cr]: https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/
[k8s-api]: https://kubernetes.io/docs/concepts/overview/kubernetes-api/
[diskpool]: https://openebs.io/docs/user-guides/replicated-storage-user-guide/replicated-pv-mayastor/rs-configuration
